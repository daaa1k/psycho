use psycho::{DocumentError, ExternalState, PresentationDocument};
use std::fs;

#[test]
fn retreat_draft_failure_preserves_blocked_document_and_retry_adopts_source() {
    for external in [
        Some("presentation { metadata { title \"External\" }; slide {} }"),
        Some("invalid {"),
        None,
    ] {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("original.kdl");
        let original = "presentation { metadata { title \"Original\" }; slide {} }";
        fs::write(&path, original).unwrap();
        let mut document = PresentationDocument::open(&path).unwrap();
        document.set_title("Retained").unwrap();
        document.set_title("Future").unwrap();
        document.undo();
        if let Some(external) = external {
            fs::write(&path, external).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        let _ = document.synchronize_external(true);
        match external {
            Some("invalid {") => assert!(matches!(
                document.external_state(),
                ExternalState::Invalid(_)
            )),
            Some(_) => assert!(matches!(document.external_state(), ExternalState::Conflict)),
            None => assert!(matches!(
                document.external_state(),
                ExternalState::Unavailable { .. }
            )),
        }
        let source = document.source().to_owned();
        let undo = document.undo_description().map(str::to_owned);
        let redo = document.redo_description().map(str::to_owned);
        let state = format!("{:?}", document.external_state());
        let draft = source.replace("Retained", "Input draft");
        assert!(document.save_as_source_overwriting(&path, &draft).is_err());
        assert!(
            document
                .save_as_source_overwriting(folder.path().join("missing/copy.kdl"), &draft)
                .is_err()
        );
        assert!(
            document
                .save_as_source_overwriting(folder.path().join("copy.kdl"), "invalid {")
                .is_err()
        );
        assert_eq!(document.source(), source);
        assert_eq!(document.path(), Some(path.as_path()));
        assert!(document.is_dirty());
        assert_eq!(document.undo_description().map(str::to_owned), undo);
        assert_eq!(document.redo_description().map(str::to_owned), redo);
        assert_eq!(format!("{:?}", document.external_state()), state);
        let destination = folder.path().join("copy.kdl");
        fs::write(&destination, "approved previous bytes").unwrap();
        assert!(matches!(
            document.save_as(&destination),
            Err(DocumentError::DestinationExists)
        ));
        document
            .save_as_source_overwriting(&destination, &draft)
            .unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), draft);
        assert_eq!(document.path(), Some(destination.as_path()));
        assert!(!document.is_dirty());
        assert!(document.can_edit());
        assert!(!document.can_undo() && !document.can_redo());
        document.set_title("Next save").unwrap();
        document.save().unwrap();
        assert!(
            fs::read_to_string(&destination)
                .unwrap()
                .contains("Next save")
        );
        assert_eq!(fs::read_to_string(&path).ok(), external.map(str::to_owned));
    }
}

#[cfg(unix)]
#[test]
fn retreat_rejects_original_aliases_even_after_original_is_deleted() {
    use std::os::unix::fs::symlink;
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().join("source");
    fs::create_dir(&base).unwrap();
    let path = base.join("original.kdl");
    fs::write(
        &path,
        "presentation { metadata { title \"Original\" }; slide {} }",
    )
    .unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    let hardlink = folder.path().join("hardlink.kdl");
    fs::hard_link(&path, &hardlink).unwrap();
    let link = folder.path().join("link.kdl");
    symlink(&path, &link).unwrap();
    let parent_link = folder.path().join("parent");
    symlink(&base, &parent_link).unwrap();
    for deleted in [false, true] {
        if deleted {
            fs::remove_file(&path).unwrap();
        }
        for destination in [
            &path,
            &hardlink,
            &link,
            &parent_link.join("original.kdl"),
            &base.join("../source/original.kdl"),
        ] {
            assert!(
                matches!(
                    document.save_as_overwriting(destination),
                    Err(DocumentError::OriginalDestination)
                ),
                "{}",
                destination.display()
            );
        }
        assert!(fs::read_to_string(&hardlink).unwrap().contains("Original"));
    }
}

#[cfg(unix)]
#[test]
fn retreat_preserves_symlink_asset_resolution_missing_assets_and_unrelated_bytes() {
    use std::os::unix::fs::symlink;
    let folder = tempfile::tempdir().unwrap();
    let old = folder.path().join("source");
    let new = folder.path().join("archive/deep");
    let real = folder.path().join("real/sub");
    for directory in [&old, &new, &real] {
        fs::create_dir_all(directory).unwrap();
    }
    fs::write(folder.path().join("real/pic.png"), b"actual asset").unwrap();
    fs::write(old.join("pic.png"), b"wrong lexical asset").unwrap();
    symlink(&real, old.join("link")).unwrap();
    let source = "presentation { metadata { title \"Original\" }; slide { image #\"link/../pic.png\"#; /- image #\"ignored.png\"#\n columns { column width=50 { image \"欠損 画像.png\"; }; column width=50 { image \"pic.png\"; }; }; }; }";
    let path = old.join("original.kdl");
    fs::write(&path, source).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    assert!(document.model().is_some(), "{:?}", document.diagnostics());
    document.save_as(old.join("same.kdl")).unwrap();
    assert_eq!(document.source(), source);
    document.save_as(new.join("copy.kdl")).unwrap();
    let paths = document.model().unwrap().slides[0].image_paths();
    assert_eq!(fs::read(new.join(&paths[0])).unwrap(), b"actual asset");
    assert!(!new.join(&paths[1]).exists());
    assert_eq!(
        fs::read(new.join(&paths[2])).unwrap(),
        b"wrong lexical asset"
    );
    assert!(document.source().contains("/- image #\"ignored.png\"#"));
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

#[cfg(unix)]
#[test]
fn retreat_rejects_symlink_cycles_without_losing_draft() {
    use std::os::unix::fs::symlink;
    let folder = tempfile::tempdir().unwrap();
    let original = folder.path().join("original.kdl");
    fs::write(
        &original,
        "presentation { metadata { title \"Original\" }; slide {} }",
    )
    .unwrap();
    let mut document = PresentationDocument::open(&original).unwrap();
    document.set_title("Draft").unwrap();
    let first = folder.path().join("first");
    let second = folder.path().join("second");
    symlink(&second, &first).unwrap();
    symlink(&first, &second).unwrap();
    assert!(document.save_as_overwriting(&first).is_err());
    assert!(document.source().contains("Draft"));
    assert!(document.undo());
    assert!(document.source().contains("Original"));
}

#[test]
fn retreat_survives_deleted_original_folder() {
    let folder = tempfile::tempdir().unwrap();
    let old = folder.path().join("deleted");
    fs::create_dir(&old).unwrap();
    let original = old.join("original.kdl");
    let source = "presentation { metadata { title \"Original\" }; slide {} }";
    fs::write(&original, source).unwrap();
    let mut document = PresentationDocument::open(&original).unwrap();
    fs::remove_dir_all(&old).unwrap();
    let destination = folder.path().join("copy.kdl");
    document.save_as(&destination).unwrap();
    assert_eq!(fs::read_to_string(&destination).unwrap(), source);
}
