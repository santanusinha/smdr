//! Tests for markdown parsing helpers (slug generation, link/TOC extraction).

use smdr::markdown::{
    extract_atx_heading, extract_links, extract_toc, github_slug, normalize_for_match,
};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// github_slug
// ---------------------------------------------------------------------------

#[test]
fn slug_simple_heading() {
    let mut seen = HashMap::new();
    assert_eq!(
        github_slug("Severity Legend", &mut seen),
        "#severity-legend"
    );
}

#[test]
fn slug_apostrophe_removed() {
    let mut seen = HashMap::new();
    assert_eq!(
        github_slug("What's Done Well", &mut seen),
        "#whats-done-well"
    );
}

#[test]
fn slug_preserves_consecutive_hyphens() {
    let mut seen = HashMap::new();
    assert_eq!(
        github_slug("Part I — Rust Idioms & Anti-patterns", &mut seen),
        "#part-i--rust-idioms--anti-patterns"
    );
}

#[test]
fn slug_strips_backticks_keeps_content() {
    let mut seen = HashMap::new();
    assert_eq!(
        github_slug("`run_async` spawns a runtime", &mut seen),
        "#run_async-spawns-a-runtime"
    );
}

#[test]
fn slug_preserves_underscores() {
    let mut seen = HashMap::new();
    assert_eq!(
        github_slug("`let _ =` silences errors", &mut seen),
        "#let-_--silences-errors"
    );
}

#[test]
fn slug_emoji_and_special_chars_removed() {
    let mut seen = HashMap::new();
    assert_eq!(
        github_slug(
            "I-1 \u{1f534} `HeapError::Success` \u{2014} success state inside an error enum",
            &mut seen
        ),
        "#i-1--heaperrorsuccess--success-state-inside-an-error-enum"
    );
}

#[test]
fn slug_deduplication() {
    let mut seen = HashMap::new();
    assert_eq!(github_slug("Heading", &mut seen), "#heading");
    assert_eq!(github_slug("Heading", &mut seen), "#heading-0");
    assert_eq!(github_slug("Heading", &mut seen), "#heading-1");
}

// ---------------------------------------------------------------------------
// extract_atx_heading
// ---------------------------------------------------------------------------

#[test]
fn extract_atx_heading_basic() {
    assert_eq!(extract_atx_heading("## Hello World"), Some("Hello World"));
    assert_eq!(extract_atx_heading("### Foo"), Some("Foo"));
    assert_eq!(extract_atx_heading("Not a heading"), None);
    assert_eq!(extract_atx_heading("####### Too many"), None);
}

#[test]
fn extract_atx_heading_trailing_hashes() {
    assert_eq!(extract_atx_heading("## Title ##"), Some("Title"));
}

// ---------------------------------------------------------------------------
// normalize_for_match
// ---------------------------------------------------------------------------

#[test]
fn normalize_strips_non_alphanumeric() {
    assert_eq!(normalize_for_match("severity-legend"), "severitylegend");
    assert_eq!(
        normalize_for_match("i-1--heaperror-success--success-state-inside-an-error-enum"),
        "i1heaperrorsuccesssuccessstateinsideanerrorenum"
    );
}

#[test]
fn anchor_match_relaxed_handles_nonstandard_slug() {
    let heading = "I-1 \u{1f534} `HeapError::Success` \u{2014} success state inside an error enum";
    let anchor = "i-1--heaperror-success--success-state-inside-an-error-enum";

    let anchor_normalized = normalize_for_match(&anchor.to_lowercase());
    let heading_normalized = normalize_for_match(&heading.replace('`', "").to_lowercase());
    assert_eq!(anchor_normalized, heading_normalized);
}

// ---------------------------------------------------------------------------
// extract_links
// ---------------------------------------------------------------------------

#[test]
fn extract_links_finds_inline_links() {
    let md = "Hello [world](https://example.com) and [foo](./bar.md)";
    let links = extract_links(md);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].text, "world");
    assert_eq!(links[0].url, "https://example.com");
    assert_eq!(links[1].text, "foo");
    assert_eq!(links[1].url, "./bar.md");
}

#[test]
fn extract_links_finds_anchor_links() {
    let md = "- [Section One](#section-one)\n- [Section Two](#section-two)\n";
    let links = extract_links(md);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].url, "#section-one");
    assert_eq!(links[1].url, "#section-two");
    assert_eq!(links[0].line, 0);
    assert_eq!(links[1].line, 1);
}

#[test]
fn extract_links_with_code_in_text() {
    let md = "See [`Config`](./config.md) for details.";
    let links = extract_links(md);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].text, "`Config`");
    assert_eq!(links[0].url, "./config.md");
}

// ---------------------------------------------------------------------------
// extract_toc
// ---------------------------------------------------------------------------

#[test]
fn extract_toc_basic() {
    let md =
        "# Title\n\nSome text\n\n## Section One\n\nContent\n\n### Subsection\n\n## Section Two\n";
    let toc = extract_toc(md);
    assert_eq!(toc.len(), 4);
    assert_eq!(toc[0].level, 1);
    assert_eq!(toc[0].text, "Title");
    assert_eq!(toc[0].line, 0);
    assert_eq!(toc[1].level, 2);
    assert_eq!(toc[1].text, "Section One");
    assert_eq!(toc[1].line, 4);
    assert_eq!(toc[2].level, 3);
    assert_eq!(toc[2].text, "Subsection");
    assert_eq!(toc[2].line, 8);
    assert_eq!(toc[3].level, 2);
    assert_eq!(toc[3].text, "Section Two");
    assert_eq!(toc[3].line, 10);
}

#[test]
fn extract_toc_strips_backticks() {
    let md = "## `Config` options\n";
    let toc = extract_toc(md);
    assert_eq!(toc.len(), 1);
    assert_eq!(toc[0].text, "Config options");
}

#[test]
fn extract_toc_skips_non_headings() {
    let md = "Not a heading\n#nospace\n####### Too many\n## Valid\n";
    let toc = extract_toc(md);
    assert_eq!(toc.len(), 1);
    assert_eq!(toc[0].text, "Valid");
    assert_eq!(toc[0].line, 3);
}

#[test]
fn extract_toc_trailing_hashes() {
    let md = "## Title ##\n";
    let toc = extract_toc(md);
    assert_eq!(toc.len(), 1);
    assert_eq!(toc[0].text, "Title");
}
