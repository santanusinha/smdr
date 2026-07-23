//! Pure, headless core for smdr's review/annotation mode.
//!
//! This module never touches iced or the GUI. It owns the annotation data
//! model (`Annotation`, `ReviewEnvelope`) and the three output serializers
//! (`render_json`, `render_annotated_md`, `render_diff`).
//! The CLI (and, later, the GUI) only *call* into here.

use serde::{Deserialize, Serialize};

/// What the human wants the agent to DO with an annotated line/section.
/// Mirrors §5 of the design doc. Keep this a small, closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Explicit thumbs-up on a section.
    Accept,
    /// Cancel/kill this item (e.g. a TODO line).
    Reject,
    /// Chose among options the agent offered; see `value`.
    Choice,
    /// Freeform feedback anchored to a line/section.
    Note,
}

impl Kind {
    /// All variants in display order — used for the composer `pick_list`.
    pub const ALL: [Kind; 4] = [Kind::Note, Kind::Accept, Kind::Reject, Kind::Choice];
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kind::Accept => f.write_str("accept"),
            Kind::Reject => f.write_str("reject"),
            Kind::Choice => f.write_str("choice"),
            Kind::Note => f.write_str("note"),
        }
    }
}

/// One comment anchored to the cut under review. Mirrors §4/§5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annotation {
    /// Source line number (0-based) in the cut being reviewed.
    pub line: usize,
    /// Optional end line (0-based, inclusive) for a range/section comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    /// The machine-actionable intent.
    pub kind: Kind,
    /// Freeform comment text. Optional (e.g. a bare `accept` may have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// For `Kind::Choice`: which option was picked (e.g. "B").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// The full result of one review turn. Mirrors the §5 "turn-level envelope".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewEnvelope {
    /// Schema tag so consumers can version-gate.
    pub schema: String,
    /// The file under review (for the agent's bookkeeping).
    pub file: String,
    /// Opaque id the agent set when it launched smdr (may be empty).
    #[serde(default)]
    pub cut_id: String,
    /// "submitted" | "cancelled".
    pub status: String,
    /// The comments authored this turn.
    pub annotations: Vec<Annotation>,
}

/// The schema tag every envelope carries. Bump on breaking changes.
pub const SCHEMA_TAG: &str = "smdr.review/v1";

/// Output serializer for a completed review turn. Shared by the CLI (`--format`)
/// and the GUI submit so both render identically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Annotated markdown: whole doc + inline notes (self-contained).
    Md,
    /// Structured JSON envelope (for harnesses that branch on kind).
    Json,
    /// Unified-diff review transport (sparse; base-in-context). DEFAULT.
    #[default]
    Diff,
}

impl ReviewEnvelope {
    /// Build a submitted envelope for `file` with `annotations`.
    pub fn submitted(file: impl Into<String>, annotations: Vec<Annotation>) -> Self {
        Self {
            schema: SCHEMA_TAG.to_string(),
            file: file.into(),
            cut_id: String::new(),
            status: "submitted".to_string(),
            annotations,
        }
    }
}

// ---------------------------------------------------------------------------
// Serializers
// ---------------------------------------------------------------------------

/// Serialize a review turn as the canonical JSON envelope (pretty-printed).
/// This is the structured form harnesses branch on. Mirrors §6 "json".
pub fn render_json(env: &ReviewEnvelope) -> String {
    // Pretty form mirrors persist.rs (to_string_pretty). Never panics on our
    // own types; fall back to an empty object string on the impossible error.
    serde_json::to_string_pretty(env).unwrap_or_else(|_| "{}".to_string())
}

/// Render one annotation as its inline HTML-comment marker (§6).
/// Invisible if the md is re-rendered, yet greppable. Example:
///   <!-- smdr[reject]: don't do this one -->
fn marker(a: &Annotation) -> String {
    let kind = match a.kind {
        Kind::Accept => "accept",
        Kind::Reject => "reject",
        Kind::Choice => "choice",
        Kind::Note => "note",
    };
    let mut body = String::new();
    if let Some(v) = &a.value {
        body.push_str(&format!("={v}"));
    }
    if let Some(c) = &a.comment {
        body.push_str(": ");
        body.push_str(c);
    }
    format!("<!-- smdr[{kind}]{body} -->")
}

/// Re-emit the WHOLE source doc with each annotation woven in as an inline
/// marker on the line AFTER its anchored `line`. Never mutates source lines —
/// it only inserts marker lines. Mirrors §6 "annotated-md".
///
/// `source` is the exact draft under review.
pub fn render_annotated_md(source: &str, env: &ReviewEnvelope) -> String {
    use std::collections::BTreeMap;
    // line index -> markers to emit right after that line. BTreeMap keeps the
    // output deterministic; a Vec per line preserves annotation order.
    let mut by_line: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for a in &env.annotations {
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
    // Annotations whose line is past EOF (defensive): append at the end.
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
/// pure INSERTION of `+ <!-- smdr[...] -->` lines surrounded by context; no
/// source line is ever removed or changed. Generated directly from the known
/// insertion positions — no diff library, no base file needed to GENERATE it.
pub fn render_diff(source: &str, env: &ReviewEnvelope) -> String {
    use std::collections::BTreeMap;
    let lines: Vec<&str> = source.lines().collect();
    let n = lines.len();

    let mut by_line: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for a in &env.annotations {
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
        ReviewEnvelope::submitted(
            "draft.md",
            vec![
                Annotation {
                    line: 8,
                    end_line: None,
                    kind: Kind::Accept,
                    comment: None,
                    value: None,
                },
                Annotation {
                    line: 14,
                    end_line: None,
                    kind: Kind::Choice,
                    comment: Some("go with B".into()),
                    value: Some("B".into()),
                },
                Annotation {
                    line: 31,
                    end_line: Some(33),
                    kind: Kind::Reject,
                    comment: Some("don't do this one".into()),
                    value: None,
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
    fn submitted_sets_schema_and_status() {
        let env = ReviewEnvelope::submitted("x.md", vec![]);
        assert_eq!(env.schema, "smdr.review/v1");
        assert_eq!(env.status, "submitted");
    }

    #[test]
    fn kind_serializes_lowercase() {
        let a = Annotation {
            line: 0,
            end_line: None,
            kind: Kind::Reject,
            comment: None,
            value: None,
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"kind\":\"reject\""), "got: {json}");
    }

    #[test]
    fn render_json_contains_schema_and_annotations() {
        let out = render_json(&sample());
        assert!(out.contains("\"schema\": \"smdr.review/v1\""));
        assert!(out.contains("\"kind\": \"choice\""));
        assert!(out.contains("\"value\": \"B\""));
        // accept annotation has no comment → field omitted
        assert!(!out.contains("\"comment\": null"));
    }

    const DOC: &str = "# Title\nalpha\nbravo\ncharlie\ndelta\necho\nfoxtrot\ngolf\nhotel\n";

    fn one(line: usize, kind: Kind, comment: &str) -> ReviewEnvelope {
        ReviewEnvelope::submitted(
            "draft.md",
            vec![Annotation {
                line,
                end_line: None,
                kind,
                comment: Some(comment.into()),
                value: None,
            }],
        )
    }

    #[test]
    fn annotated_md_inserts_marker_after_line_and_preserves_source() {
        let env = one(2, Kind::Reject, "drop this");
        let out = render_annotated_md(DOC, &env);
        // every original line still present, in order
        for l in DOC.lines() {
            assert!(out.contains(l), "missing source line: {l}");
        }
        // marker appears immediately after line 2 ("bravo")
        let pos_bravo = out.find("bravo\n").unwrap();
        let pos_marker = out.find("<!-- smdr[reject]: drop this -->").unwrap();
        assert!(pos_marker > pos_bravo);
        // source line count unchanged among non-marker lines
        let non_marker: Vec<&str> = out
            .lines()
            .filter(|l| !l.starts_with("<!-- smdr["))
            .collect();
        assert_eq!(non_marker, DOC.lines().collect::<Vec<_>>());
    }

    #[test]
    fn diff_is_insertion_only_with_context() {
        let env = one(4, Kind::Note, "tighten");
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
                assert!(rest.starts_with("<!-- smdr["), "unexpected added line: {l}");
            }
            assert!(
                !l.starts_with('-') || l.starts_with("---"),
                "diff must not remove source lines: {l}"
            );
        }
    }

    #[test]
    fn diff_empty_when_no_annotations() {
        let env = ReviewEnvelope::submitted("draft.md", vec![]);
        assert_eq!(render_diff(DOC, &env), "");
    }
}
