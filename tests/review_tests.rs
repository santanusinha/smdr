//! Integration tests for `--review` (headless one-shot mode).
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn smdr() -> Command {
    Command::cargo_bin("smdr").expect("smdr binary")
}

/// Write a draft + annotations file in a temp dir; return (guard, draft, annot).
fn fixture() -> (TempDir, String, String) {
    let dir = TempDir::new().expect("tempdir");
    let draft = dir.path().join("draft.md");
    let annot = dir.path().join("a.json");
    fs::write(
        &draft,
        "# Title\nalpha\nbravo\ncharlie\ndelta\necho\nfoxtrot\n",
    )
    .unwrap();
    fs::write(
        &annot,
        r#"[{"line":2,"kind":"reject","comment":"drop this"}]"#,
    )
    .unwrap();
    (
        dir,
        draft.to_string_lossy().into_owned(),
        annot.to_string_lossy().into_owned(),
    )
}

#[test]
fn review_diff_emits_insertion_only_transport() {
    let (_d, draft, annot) = fixture();
    smdr()
        .args([
            "--review",
            &draft,
            "--annotations-in",
            &annot,
            "--format",
            "diff",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("--- a/"))
        .stdout(predicate::str::contains(
            "+<!-- smdr[reject]: drop this -->",
        ))
        .stdout(predicate::str::contains("do not git-apply"));
}

#[test]
fn review_md_emits_whole_doc_with_marker() {
    let (_d, draft, annot) = fixture();
    smdr()
        .args([
            "--review",
            &draft,
            "--annotations-in",
            &annot,
            "--format",
            "md",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha"))
        .stdout(predicate::str::contains("foxtrot"))
        .stdout(predicate::str::contains("<!-- smdr[reject]: drop this -->"));
}

#[test]
fn review_json_emits_envelope() {
    let (_d, draft, annot) = fixture();
    smdr()
        .args([
            "--review",
            &draft,
            "--annotations-in",
            &annot,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema\": \"smdr.review/v1\""))
        .stdout(predicate::str::contains("\"kind\": \"reject\""));
}

#[test]
fn review_never_mutates_source() {
    let (_d, draft, annot) = fixture();
    let before = fs::read_to_string(&draft).unwrap();
    for fmt in ["diff", "md", "json"] {
        smdr()
            .args([
                "--review",
                &draft,
                "--annotations-in",
                &annot,
                "--format",
                fmt,
            ])
            .assert()
            .success();
    }
    let after = fs::read_to_string(&draft).unwrap();
    assert_eq!(before, after, "review must never modify the input file");
}

#[test]
fn review_out_flag_writes_file_and_is_silent_on_stdout() {
    let (_d, draft, annot) = fixture();
    let out = format!("{draft}.out");
    smdr()
        .args([
            "--review",
            &draft,
            "--annotations-in",
            &annot,
            "--format",
            "diff",
            "--out",
            &out,
        ])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert!(fs::read_to_string(&out).unwrap().contains("smdr[reject]"));
}

#[test]
fn review_requires_file() {
    smdr().args(["--review"]).assert().failure();
}
