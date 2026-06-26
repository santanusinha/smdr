//! Markdown parsing helpers — link extraction, TOC generation, slug creation.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A hyperlink extracted from a markdown document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLink {
    /// Source line number (0-based) where the link appears.
    pub line: usize,
    /// The link destination URL/path.
    pub url: String,
    /// Display text of the link.
    pub text: String,
}

/// A heading extracted from the document for sidebar navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    /// Heading level (1-6).
    pub level: u8,
    /// Display text of the heading.
    pub text: String,
    /// Line number (0-based) in the source.
    pub line: usize,
    /// Pre-computed GitHub-style anchor slug (without leading `#`).
    /// Used by `compute_anchor_y` to avoid re-scanning raw markdown.
    pub slug: String,
    /// Pre-computed normalized form (ASCII alphanumeric only, lowercase)
    /// for the relaxed-match fallback in anchor navigation.
    pub slug_normalized: String,
}

// ---------------------------------------------------------------------------
// Link extraction (using pulldown-cmark)
// ---------------------------------------------------------------------------

/// Extract all links from the markdown source with their line positions.
pub fn extract_links(source: &str) -> Vec<DocumentLink> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let mut links = Vec::new();
    let parser = Parser::new_ext(source, Options::all());

    // Track byte offset → line number mapping
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(i, b)| if b == b'\n' { Some(i + 1) } else { None }),
        )
        .collect();

    let byte_offset_to_line = |offset: usize| -> usize {
        line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    };

    let mut in_link: Option<(String, usize)> = None; // (url, line)
    let mut link_text = String::new();

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                let line = byte_offset_to_line(range.start);
                in_link = Some((dest_url.to_string(), line));
                link_text.clear();
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, line)) = in_link.take() {
                    // For normal [text](url) links, move link_text instead of cloning.
                    // For bare-URL links (link_text is empty), one url.clone() is
                    // still required because url is also stored in DocumentLink::url.
                    let display = if link_text.is_empty() {
                        url.clone()
                    } else {
                        std::mem::take(&mut link_text)
                    };
                    links.push(DocumentLink {
                        line,
                        url,
                        text: display,
                    });
                }
                link_text.clear();
            }
            Event::Text(t) if in_link.is_some() => {
                link_text.push_str(&t);
            }
            Event::Code(c) if in_link.is_some() => {
                link_text.push('`');
                link_text.push_str(&c);
                link_text.push('`');
            }
            _ => {}
        }
    }

    links
}

// ---------------------------------------------------------------------------
// Table of contents extraction
// ---------------------------------------------------------------------------

/// Extract all headings from the markdown source for the sidebar outline.
pub fn extract_toc(source: &str) -> Vec<TocEntry> {
    let mut entries = Vec::new();
    let mut seen: HashMap<String, u32> = HashMap::new();

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim_start_matches('#');
        let hash_count = line.len() - trimmed.len();
        if hash_count == 0 || hash_count > 6 {
            continue;
        }
        // Must have a space after the hashes
        if !trimmed.starts_with(' ') {
            continue;
        }
        let heading_text = trimmed[1..].trim_end().trim_end_matches('#').trim();
        if heading_text.is_empty() {
            continue;
        }
        // Strip backticks for display
        let display_text = heading_text.replace('`', "");
        // Compute the GitHub slug once at load time and strip the leading '#'.
        let slug_with_hash = github_slug(heading_text, &mut seen);
        let slug = slug_with_hash.trim_start_matches('#').to_owned();
        let slug_normalized = normalize_for_match(&display_text.to_lowercase());
        entries.push(TocEntry {
            level: hash_count as u8,
            text: display_text,
            line: line_num,
            slug,
            slug_normalized,
        });
    }

    entries
}

// ---------------------------------------------------------------------------
// Heading / slug helpers
// ---------------------------------------------------------------------------

/// Generate a GitHub-style anchor slug from heading text.
///
/// Rules (matching GitHub.com behaviour):
/// 1. Strip inline-code backticks (keep their content).
/// 2. Lowercase.
/// 3. Remove everything that is not ASCII alphanumeric, space, hyphen,
///    or underscore.
/// 4. Replace spaces with hyphens (consecutive hyphens are preserved).
/// 5. Deduplicate: second occurrence gets suffix `-1`, third gets `-2`, etc.
pub fn github_slug(text: &str, seen: &mut HashMap<String, u32>) -> String {
    let base: String = text
        .replace('`', "")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect();

    let count = seen.entry(base.clone()).or_insert(0);
    let slug = if *count == 0 {
        base.clone()
    } else {
        format!("{base}-{}", *count - 1)
    };
    *count += 1;
    format!("#{slug}")
}

/// Extract the heading text from an ATX heading line, or `None` if the line
/// is not a valid ATX heading.
pub fn extract_atx_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches('#');
    let hashes = line.len() - trimmed.len();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = trimmed.strip_prefix(' ')?;
    let text = rest.trim_end().trim_end_matches('#').trim_end_matches(' ');
    Some(if text.is_empty() { rest.trim() } else { text })
}

/// Strips everything except ASCII alphanumeric characters for fuzzy anchor comparison.
pub fn normalize_for_match(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}
