use std::fs;

use psycho::{DiagnosticKind, ExternalState, PresentationDocument};

const ORIGINAL: &str = "presentation {\n metadata { title \"Original\" }\n slide id=\"intro\" { heading \"Before\" }\n}\n";

#[test]
fn clean_external_change_reloads_and_clears_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("slides.kdl");
    fs::write(&path, ORIGINAL).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("Undone").unwrap();
    assert!(document.undo());
    assert!(document.can_redo());

    let external = ORIGINAL.replace("Before", "External");
    fs::write(&path, &external).unwrap();
    assert!(document.synchronize_external(false).unwrap());
    assert_eq!(document.source(), external);
    assert!(!document.is_dirty());
    assert!(!document.can_undo());
    assert!(!document.can_redo());
    assert!(document.can_edit());
    assert!(!document.synchronize_external(false).unwrap());
    document.set_title("After reload").unwrap();
    document.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), document.source());
}

#[test]
fn conflict_freezes_edits_and_history_until_explicit_reload() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("slides.kdl");
    fs::write(&path, ORIGINAL).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("GUI edit").unwrap();
    document.set_title("Redo edit").unwrap();
    assert!(document.undo());
    let edited = document.source().to_owned();
    let undo_label = document.undo_description().map(str::to_owned);
    let redo_label = document.redo_description().map(str::to_owned);
    let external = ORIGINAL.replace("Before", "External");
    fs::write(&path, &external).unwrap();

    assert!(document.synchronize_external(false).is_err());
    assert!(!document.can_edit());
    assert!(!document.can_undo());
    assert!(!document.can_redo());
    assert!(!document.undo());
    assert!(!document.redo());
    assert!(document.set_title("Forbidden").is_err());
    assert!(document.save().is_err());
    assert_eq!(document.source(), edited);
    assert_eq!(document.undo_description(), undo_label.as_deref());
    assert_eq!(document.redo_description(), redo_label.as_deref());
    assert_eq!(fs::read_to_string(&path).unwrap(), external);

    document.reload_from_disk().unwrap();
    assert_eq!(document.source(), external);
    assert!(document.can_edit());
    assert!(!document.is_dirty());
    assert_eq!(document.undo_description(), None);
    assert_eq!(document.redo_description(), None);
}

#[test]
fn invalid_external_source_preserves_edits_and_diagnoses_the_disk_version() {
    for (invalid, kind) in [
        ("presentation {", DiagnosticKind::Syntax),
        (
            "presentation {\n metadata { title \"Invalid\" }\n slide { mystery \"bad\" }\n}\n",
            DiagnosticKind::Schema,
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("slides.kdl");
        fs::write(&path, ORIGINAL).unwrap();
        let mut document = PresentationDocument::open(&path).unwrap();
        document.set_title("GUI edit").unwrap();
        document.set_title("Redo edit").unwrap();
        assert!(document.undo());
        let edited = document.source().to_owned();
        fs::write(&path, invalid).unwrap();

        assert!(document.synchronize_external(false).is_err());
        let ExternalState::Invalid(diagnostics) = document.external_state() else {
            panic!("invalid disk source must have diagnostics even with GUI edits");
        };
        assert!(!diagnostics.is_empty());
        assert!(diagnostics.iter().all(|diagnostic| diagnostic.kind == kind));
        for diagnostic in diagnostics {
            assert_eq!(
                diagnostic.source_line,
                invalid.lines().nth(diagnostic.line - 1).unwrap()
            );
            assert!(
                diagnostic
                    .file_name
                    .as_ref()
                    .unwrap()
                    .contains("slides.kdl")
            );
        }
        assert_eq!(document.model().unwrap().title, "GUI edit");
        assert!(document.reload_from_disk().is_err());
        assert_eq!(document.source(), edited);
        assert!(document.undo_description().is_some());
        assert!(document.redo_description().is_some());
        assert!(!document.undo());
        assert!(!document.redo());
        assert!(!document.can_edit());
        assert!(document.save().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), invalid);

        fs::write(&path, ORIGINAL).unwrap();
        document.reload_from_disk().unwrap();
        assert!(document.can_edit());
        assert!(matches!(document.external_state(), ExternalState::Current));
        assert_eq!(document.source(), ORIGINAL);
        assert_eq!(document.undo_description(), None);
        assert_eq!(document.redo_description(), None);
    }
}

#[test]
fn deletion_and_failed_retry_keep_the_document_and_both_history_branches() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("slides.kdl");
    fs::write(&path, ORIGINAL).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("GUI edit").unwrap();
    document.set_title("Redo edit").unwrap();
    assert!(document.undo());
    let edited = document.source().to_owned();
    fs::remove_file(&path).unwrap();

    assert!(document.synchronize_external(false).is_err());
    assert!(matches!(
        document.external_state(),
        ExternalState::Unavailable {
            kind: std::io::ErrorKind::NotFound,
            ..
        }
    ));
    assert!(!document.can_edit());
    assert!(document.reload_from_disk().is_err());
    assert!(document.save().is_err());
    assert!(!path.exists());
    assert_eq!(document.source(), edited);
    assert!(document.undo_description().is_some());
    assert!(document.redo_description().is_some());
    assert!(!document.undo());
    assert!(!document.redo());

    // Restoring the baseline must not silently unlock a blocked session.
    fs::write(&path, ORIGINAL).unwrap();
    assert!(document.synchronize_external(false).is_err());
    assert!(document.save().is_err());
    document.reload_from_disk().unwrap();
    assert!(document.can_edit());
    assert_eq!(document.source(), ORIGINAL);
    assert_eq!(document.undo_description(), None);
    assert_eq!(document.redo_description(), None);
}

#[test]
fn input_only_draft_prevents_auto_reload_and_failed_reload_keeps_redo() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("slides.kdl");
    fs::write(&path, ORIGINAL).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("Redo edit").unwrap();
    assert!(document.undo());
    assert!(!document.is_dirty());
    fs::write(&path, ORIGINAL.replace("Original", "External")).unwrap();

    assert!(document.synchronize_external(true).is_err());
    assert!(matches!(document.external_state(), ExternalState::Conflict));
    assert_eq!(document.source(), ORIGINAL);
    assert!(!document.redo());
    fs::write(&path, "presentation {").unwrap();
    assert!(document.reload_from_disk().is_err());
    assert_eq!(document.source(), ORIGINAL);
    assert!(document.redo_description().is_some());
    assert!(!document.can_edit());
    fs::write(&path, ORIGINAL).unwrap();
    document.reload_from_disk().unwrap();
    assert!(document.can_edit());
    assert_eq!(document.redo_description(), None);
}

#[test]
fn save_detects_invalid_external_source_and_freezes_history_without_polling() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("slides.kdl");
    fs::write(&path, ORIGINAL).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("GUI edit").unwrap();
    fs::write(&path, "presentation {").unwrap();

    assert!(document.save().is_err());
    assert!(!document.can_edit());
    assert!(!document.undo());
    assert!(document.undo_description().is_some());
    assert!(matches!(
        document.external_state(),
        ExternalState::Invalid(_)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), "presentation {");
}
