use bake_releases::{extract_notes, update_document};
use std::fs;

#[test]
fn preserves_nested_sections_and_markdown() {
    let document =
        "# Releases\n\n## v1.0.0\n\nHello **world**.\n\n### Features\n\n- One\n\n## v0.9.0\nOld\n";
    assert_eq!(
        extract_notes(document, "v1.0.0").unwrap(),
        "\nHello **world**.\n\n### Features\n\n- One\n\n"
    );
    assert_eq!(extract_notes(document, "v0.9.0").unwrap(), "Old\n");
}

#[test]
fn ignores_headings_in_fenced_code() {
    for fence in ["```", "~~~~", "  ```rust"] {
        let closing = if fence.contains('~') { "~~~~" } else { "```" };
        let body = format!("\n{fence}\n## Unreleased\n## v0.1.0\n{closing}\n\nText\n");
        let document = format!("## v1.0.0\n{body}## v0.1.0\nOlder\n");
        assert_eq!(extract_notes(&document, "v1.0.0").unwrap(), body);
        assert!(update_document(&document, "v2.0.0").is_err());
    }
}

#[test]
fn fences_require_a_sufficiently_long_matching_close() {
    let document = "## v1\n````\n```\n## v0\n~~~~\n## v0\n````\nDone\n## v0\nOld\n";
    assert!(extract_notes(document, "v1").unwrap().ends_with("Done\n"));
    assert_eq!(extract_notes(document, "v0").unwrap(), "Old\n");
}

#[test]
fn headings_are_exact_unique_and_unindented() {
    assert!(extract_notes("## v1.0.0\nHi\n", "1.0.0").is_err());
    assert!(extract_notes("## v1\nFirst\n## v1\nSecond\n", "v1").is_err());
    assert!(extract_notes("    ## v1\n", "v1").is_err());
    assert!(extract_notes("##v1\n", "v1").is_err());
    assert_eq!(extract_notes("## v1 ###\nHi", "v1").unwrap(), "Hi");
    assert_eq!(extract_notes("## v1", "v1").unwrap(), "");
}

#[test]
fn updates_only_the_heading_preserving_crlf_and_unicode() {
    let document = "# Releases\r\n\r\n## Unreleased ##\r\n\r\nこんにちは\r\n\r\n## v0\r\nOld\r\n";
    assert_eq!(
        update_document(document, "v1").unwrap(),
        document.replacen("Unreleased", "v1", 1)
    );
    assert_eq!(
        extract_notes(document, "Unreleased").unwrap(),
        "\r\nこんにちは\r\n\r\n"
    );
}

#[test]
fn invalid_updates_leave_no_result() {
    for version in ["", "Unreleased", " v1", "v1 ", "v1\n## Bad", "v1\0", "v1#"] {
        assert!(
            update_document("## Unreleased\n", version).is_err(),
            "{version:?}"
        );
    }
    assert!(update_document("## Unreleased\n## v1\n", "v1").is_err());
    assert!(update_document("## Unreleased\n## Unreleased\n", "v1").is_err());
    assert!(update_document("## v0\n", "v1").is_err());
}

#[test]
fn tasks_read_and_update_a_custom_document_under_project_root() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("changes.md");
    fs::write(&path, "# Releases\n\n## Unreleased\n\nNew feature.\n").unwrap();
    let registry = bake::Registry::discover().unwrap();
    let mut context = registry.context(directory.path());
    assert_eq!(
        context
            .call("releases:notes", &["Unreleased", "--path", "changes.md"])
            .unwrap(),
        "\nNew feature.\n"
    );
    context
        .call("releases:update", &["v1", "--path", "changes.md"])
        .unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("## v1\n"));
    let updated = fs::read(&path).unwrap();
    assert!(
        context
            .call("releases:update", &["v2", "--path", "changes.md"])
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), updated);
}
