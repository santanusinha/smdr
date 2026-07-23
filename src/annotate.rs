//! Pure, headless core for smdr's review/annotation mode.
//!
//! This module never touches iced or the GUI. It owns the annotation data
//! model (`Annotation`, `ReviewEnvelope`) and the three output serializers
//! (`render_json`, `render_annotated_md`, `render_diff`).
//! The CLI (and the GUI) only *call* into here.

use serde::{Deserialize, Serialize};

/// One line-anchored comment in a review turn.
///
/// The model is deliberately minimal: a review is just freeform notes pinned to
/// source lines. There are no "op types" (accept/reject/choice) — a reviewing
/// agent reads the prose and decides what to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annotation {
    /// Source line number (0-based) in the cut being reviewed.
    pub line: usize,
    /// Freeform comment text.
    pub comment: String,
}

/// The full result of one review turn: the file under review plus every
/// line-anchored comment authored this turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewEnvelope {
    /// Schema tag so consumers can version-gate.
    pub schema: String,
    /// The file under review (for the agent's bookkeeping).
    pub file: String,
    /// The comments authored this turn.
    pub comments: Vec<Annotation>,
}

/// The schema tag every envelope carries. Bump on breaking changes.
pub const SCHEMA_TAG: &str = "smdr.review/v1";

/// Output serializer for a completed review turn. Shared by the CLI (`--format`)
/// and the GUI submit so both render identically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Annotated markdown: whole doc + inline notes (self-contained).
    Md,
    /// Structured JSON envelope. DEFAULT.
    #[default]
    Json,
    /// Unified-diff review transport (sparse; base-in-context).
    Diff,
}

impl ReviewEnvelope {
    /// Build an envelope for `file` carrying `comments`.
    pub fn new(file: impl Into<String>, comments: Vec<Annotation>) -> Self {
        Self {
            schema: SCHEMA_TAG.to_string(),
            file: file.into(),
            comments,
        }
    }
}

// ---------------------------------------------------------------------------
// Serializers
// ---------------------------------------------------------------------------

/// Serialize a review turn as the canonical JSON envelope (pretty-printed).
/// This is the structured form harnesses consume. Mirrors §6 "json".
pub fn render_json(env: &ReviewEnvelope) -> String {
    // Pretty form mirrors persist.rs (to_string_pretty). Never panics on our
    // own types; fall back to an empty object string on the impossible error.
    serde_json::to_string_pretty(env).unwrap_or_else(|_| "{}".to_string())
}

/// Render one comment as its inline HTML-comment marker (§6).
/// Invisible if the md is re-rendered, yet greppable. Example:
///   <!-- smdr: tighten this paragraph -->
fn marker(a: &Annotation) -> String {
    format!("<!-- smdr: {} -->", a.comment)
}

/// Re-emit the WHOLE source doc with each comment woven in as an inline marker
/// on the line AFTER its anchored `line`. Never mutates source lines — it only
/// inserts marker lines. Mirrors §6 "annotated-md".
///
/// `source` is the exact draft under review.
pub fn render_annotated_md(source: &str, env: &ReviewEnvelope) -> String {
    use std::collections::BTreeMap;
    // line index -> markers to emit right after that line. BTreeMap keeps the
    // output deterministic; a Vec per line preserves comment order.
    let mut by_line: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for a in &env.comments {
        by_line.entry(a.line).or_default().push(marker(a));
    }

    let mut out = String::new();
    for (i, line) in source.lines().enumerate() {
        out.push_str(line);
        out.push('\n');
        if let Some(markers) = by_line.get(&i) {
            for m in markers {
                out.push_str(m);
                out.push('\n');
            }
        }
    }
    // Comments whose line is past EOF (defensive): append at the end.
    let max_line = source.lines().count();
    for (_line, markers) in by_line.range(max_line..) {
        for m in markers {
            out.push_str(m);
            out.push('\n');
        }
    }
    out
}

/// Number of unchanged context lines to emit on each side of an inserted
/// comment in `render_diff`. Generous on purpose (§6): the consumer is an LLM
/// locating by content, not `patch` applying by offset.
pub const DIFF_CONTEXT: usize = 6;

/// Render a unified-diff-style review transport (§6 "diff").
///
/// IMPORTANT: this is REVIEW TRANSPORT, not a patch to apply. Every hunk is a
/// pure INSERTION of `+ <!-- smdr: ... -->` lines surrounded by context; no
/// source line is ever removed or changed. Generated directly from the known
/// insertion positions — no diff library, no base file needed to GENERATE it.
pub fn render_diff(source: &str, env: &ReviewEnvelope) -> String {
    use std::collections::BTreeMap;
    let lines: Vec<&str> = source.lines().collect();
    let n = lines.len();

    let mut by_line: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for a in &env.comments {
        let anchor = a.line.min(n.saturating_sub(1));
        by_line.entry(anchor).or_default().push(marker(a));
    }
    if by_line.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("--- a/");
    out.push_str(&env.file);
    out.push('\n');
    out.push_str("+++ b/");
    out.push_str(&env.file);
    out.push_str("  (smdr review — INSERTIONS ONLY, do not git-apply)\n");

    // One hunk per annotated anchor, with DIFF_CONTEXT lines of context.
    for (&anchor, markers) in &by_line {
        let start = anchor.saturating_sub(DIFF_CONTEXT);
        let end = (anchor + DIFF_CONTEXT + 1).min(n); // exclusive
        let ctx_len = end - start;
        // old block = ctx_len lines; new block = ctx_len + inserted markers
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            start + 1,
            ctx_len,
            start + 1,
            ctx_len + markers.len()
        ));
        for (idx, line_text) in lines[start..end].iter().enumerate() {
            out.push(' ');
            out.push_str(line_text);
            out.push('\n');
            if start + idx == anchor {
                for m in markers {
                    out.push('+');
                    out.push_str(m);
                    out.push('\n');
                }
            }
        }
    }
    out
}

/// Render one review turn in the requested [`OutputFormat`].
///
/// A single dispatcher shared by the headless CLI path (`--annotations-in`) and
/// the interactive GUI submit, so both honour `--format` identically.
pub fn render(source: &str, env: &ReviewEnvelope, format: OutputFormat) -> String {
    match format {
        OutputFormat::Md => render_annotated_md(source, env),
        OutputFormat::Json => render_json(env),
        OutputFormat::Diff => render_diff(source, env),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ReviewEnvelope {
        ReviewEnvelope::new(
            "draft.md",
            vec![
                Annotation {
                    line: 8,
                    comment: "looks good".into(),
                },
                Annotation {
                    line: 14,
                    comment: "go with B".into(),
                },
                Annotation {
                    line: 31,
                    comment: "don't do this one".into(),
                },
            ],
        )
    }

    #[test]
    fn envelope_json_roundtrips() {
        let env = sample();
        let json = serde_json::to_string(&env).expect("serialize");
        let back: ReviewEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(env, back);
    }

    #[test]
    fn new_sets_schema() {
        let env = ReviewEnvelope::new("x.md", vec![]);
        assert_eq!(env.schema, "smdr.review/v1");
        assert_eq!(env.file, "x.md");
        assert!(env.comments.is_empty());
    }

    #[test]
    fn render_json_contains_schema_and_comments() {
        let out = render_json(&sample());
        assert!(out.contains("\"schema\": \"smdr.review/v1\""));
        assert!(out.contains("\"comments\""));
        assert!(out.contains("\"comment\": \"go with B\""));
        // lean model: no op-type/kind, value, end_line, cut_id, or status fields
        assert!(!out.contains("\"kind\""));
        assert!(!out.contains("\"value\""));
        assert!(!out.contains("\"end_line\""));
        assert!(!out.contains("\"cut_id\""));
        assert!(!out.contains("\"status\""));
    }

    const DOC: &str = "# Title\nalpha\nbravo\ncharlie\ndelta\necho\nfoxtrot\ngolf\nhotel\n";

    fn one(line: usize, comment: &str) -> ReviewEnvelope {
        ReviewEnvelope::new(
            "draft.md",
            vec![Annotation {
                line,
                comment: comment.into(),
            }],
        )
    }

    #[test]
    fn annotated_md_inserts_marker_after_line_and_preserves_source() {
        let env = one(2, "drop this");
        let out = render_annotated_md(DOC, &env);
        // every original line still present, in order
        for l in DOC.lines() {
            assert!(out.contains(l), "missing source line: {l}");
        }
        // marker appears immediately after line 2 ("bravo")
        let pos_bravo = out.find("bravo\n").unwrap();
        let pos_marker = out.find("<!-- smdr: drop this -->").unwrap();
        assert!(pos_marker > pos_bravo);
        // source line count unchanged among non-marker lines
        let non_marker: Vec<&str> = out
            .lines()
            .filter(|l| !l.starts_with("<!-- smdr"))
            .collect();
        assert_eq!(non_marker, DOC.lines().collect::<Vec<_>>());
    }

    #[test]
    fn diff_is_insertion_only_with_context() {
        let env = one(4, "tighten");
        let out = render_diff(DOC, &env);
        assert!(out.starts_with("--- a/draft.md\n"));
        assert!(out.contains("do not git-apply"));
        assert!(out.contains("@@ "));
        // the ONLY added lines are smdr markers; nothing is removed
        for l in out.lines() {
            if let Some(rest) = l.strip_prefix('+') {
                if rest.starts_with("++") {
                    continue;
                } // the +++ header
                assert!(rest.starts_with("<!-- smdr"), "unexpected added line: {l}");
            }
            assert!(
                !l.starts_with('-') || l.starts_with("---"),
                "diff must not remove source lines: {l}"
            );
        }
    }

    #[test]
    fn diff_empty_when_no_comments() {
        let env = ReviewEnvelope::new("draft.md", vec![]);
        assert_eq!(render_diff(DOC, &env), "");
    }
}
