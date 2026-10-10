use std::fs;

use psycho::{
    Element, ExternalState, LayoutDiagnostic, PresentationDocument, PresentationStart,
    PresentationStartError, SlidePosition,
};

const SOURCE: &str = "presentation {\n metadata { title \"Original\" }\n slide id=\"intro\" { heading \"Before\" }\n slide id=\"last\" { text \"Last\" }\n}\n";

#[test]
fn unsaved_snapshot_preserves_disk_history_and_navigation_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("slides.kdl");
    fs::write(&path, SOURCE).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("Unsaved").unwrap();
    let mut session = document
        .start_presentation(PresentationStart::First, |_| vec![])
        .unwrap();
    assert_eq!(session.model().title, "Unsaved");
    assert!(document.is_dirty());
    assert!(document.can_undo());
    assert_eq!(fs::read_to_string(&path).unwrap(), SOURCE);
    session.previous();
    assert_eq!(session.current_index(), 0);
    session.next();
    session.next();
    assert_eq!(session.current_index(), 1);
    document.set_title("After").unwrap();
    fs::remove_file(&path).unwrap();
    assert_eq!(session.model().title, "Unsaved");
    assert_eq!(
        session.position().resolve(document.model().unwrap()),
        Some(1)
    );
    assert!(document.undo());
    assert_eq!(document.model().unwrap().title, "Unsaved");
}

#[test]
fn current_start_follows_identity_across_clean_external_reload() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("slides.kdl");
    fs::write(&path, SOURCE).unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    fs::write(&path, "presentation {\n metadata { title \"New\" }\n slide id=\"last\" { text \"Last\" }\n slide id=\"intro\" { heading \"Moved\" }\n}\n").unwrap();
    let session = document
        .start_presentation(PresentationStart::Current(0), |_| vec![])
        .unwrap();
    assert_eq!(session.current_index(), 1);
    assert_eq!(session.model().title, "New");
    assert_eq!(document.model().unwrap().title, "New");
}

#[test]
fn position_falls_back_to_order_then_last_then_none() {
    let document = PresentationDocument::from_source(SOURCE).unwrap();
    let model = document.model().unwrap();
    let position = SlidePosition::capture(model, 1);
    let mut replacement = model.clone();
    replacement.slides[1].id = Some("replacement".into());
    assert_eq!(position.resolve(&replacement), Some(1));
    replacement.slides.pop();
    assert_eq!(position.resolve(&replacement), Some(0));
    replacement.slides.clear();
    assert_eq!(position.resolve(&replacement), None);
}

#[test]
fn empty_invalid_and_all_slide_layout_validation_prevent_start() {
    let mut empty =
        PresentationDocument::from_source("presentation { metadata { title \"Empty\" } }\n")
            .unwrap();
    assert!(matches!(
        empty.start_presentation(PresentationStart::First, |_| panic!("empty")),
        Err(PresentationStartError::Empty)
    ));
    let mut invalid = PresentationDocument::from_source("presentation {").unwrap();
    assert!(matches!(
        invalid.start_presentation(PresentationStart::First, |_| panic!("invalid")),
        Err(PresentationStartError::Document(_))
    ));
    let mut document = PresentationDocument::from_source(SOURCE).unwrap();
    let result = document.start_presentation(PresentationStart::First, |model| {
        assert_eq!(model.slides.len(), 2);
        vec![LayoutDiagnostic {
            slide_index: 1,
            element_index: 0,
            message: "overflow".into(),
        }]
    });
    assert!(
        matches!(result, Err(PresentationStartError::Layout(issues)) if issues[0].slide_index == 1)
    );
}

#[test]
fn external_conflict_invalid_and_deleted_files_prevent_start() {
    for change in ["conflict", "invalid", "deleted"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("slides.kdl");
        fs::write(&path, SOURCE).unwrap();
        let mut document = PresentationDocument::open(&path).unwrap();
        match change {
            "conflict" => {
                document.set_title("Unsaved").unwrap();
                fs::write(&path, SOURCE.replace("Original", "External")).unwrap();
            }
            "invalid" => fs::write(&path, "presentation {").unwrap(),
            _ => fs::remove_file(&path).unwrap(),
        }
        assert!(matches!(
            document.start_presentation(PresentationStart::First, |_| panic!("external error")),
            Err(PresentationStartError::Document(_))
        ));
        assert!(!matches!(document.external_state(), ExternalState::Current));
    }
}

#[test]
fn image_pixels_are_frozen_after_update_and_delete() {
    let directory = tempfile::tempdir().unwrap();
    let image_path = directory.path().join("image.png");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([10, 20, 30, 255]))
        .save(&image_path)
        .unwrap();
    let source =
        "presentation {\n metadata { title \"Images\" }\n slide { image \"image.png\" }\n}\n";
    let mut document =
        PresentationDocument::from_source_with_asset_base(source, directory.path()).unwrap();
    let session = document
        .start_presentation(PresentationStart::First, |_| vec![])
        .unwrap();
    image::RgbaImage::from_pixel(1, 1, image::Rgba([50, 60, 70, 255]))
        .save(&image_path)
        .unwrap();
    assert_eq!(
        session.images()["image.png"].get_pixel(0, 0).0,
        [10, 20, 30, 255]
    );
    fs::remove_file(&image_path).unwrap();
    assert_eq!(
        session.images()["image.png"].get_pixel(0, 0).0,
        [10, 20, 30, 255]
    );
    assert!(matches!(
        document.start_presentation(PresentationStart::First, |_| vec![]),
        Err(PresentationStartError::Assets(_))
    ));
    assert!(matches!(
        session.model().slides[0].elements[0],
        Element::Image { .. }
    ));
}

#[test]
fn nested_images_on_other_slides_are_all_checked() {
    let directory = tempfile::tempdir().unwrap();
    let source = "presentation {\n metadata { title \"Images\" }\n slide { heading \"First\" }\n slide { columns {\n column width=50 { image \"missing-left.png\" }\n column width=50 { image \"missing-right.png\" }\n } }\n}\n";
    let mut document =
        PresentationDocument::from_source_with_asset_base(source, directory.path()).unwrap();
    assert!(document.model().is_some(), "{:?}", document.diagnostics());
    assert!(
        matches!(document.start_presentation(PresentationStart::First, |_| vec![]), Err(PresentationStartError::Assets(issues)) if issues.len() == 2 && issues.iter().all(|issue| issue.slide_index == 1 && issue.column_index.is_some()))
    );
}
