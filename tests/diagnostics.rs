use psycho::{DiagnosticKind, PresentationDocument};

#[test]
fn diagnostics_follow_every_kdl_newline_in_a_mixed_document() {
    let separators = [
        "\r\n", "\r", "\n", "\u{85}", "\u{2028}", "\u{2029}", "\u{c}",
    ];
    let mut source =
        String::from("presentation {\n metadata { title \"日本語\" }\n slide id=\"mixed\" {");
    for newline in separators {
        source.push_str(newline);
        source.push_str("  text 42");
    }
    source.push_str("\n }\n}\n");
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("mixed.kdl");
    std::fs::write(&path, &source).unwrap();
    let document = PresentationDocument::open(&path).unwrap();
    assert!(document.model().is_none());
    assert!(!document.can_edit());
    assert_eq!(document.diagnostics().len(), separators.len());
    for (index, diagnostic) in document.diagnostics().iter().enumerate() {
        assert_eq!(diagnostic.kind, DiagnosticKind::Schema);
        assert_eq!(diagnostic.file_name.as_deref(), Some("mixed.kdl"));
        assert_eq!(diagnostic.line, index + 4, "{diagnostic:?}");
        assert_eq!(diagnostic.column, 3);
        assert_eq!(diagnostic.source_line, "  text 42");
        assert_eq!(diagnostic.slide_index, Some(0));
        assert_eq!(diagnostic.slide_id.as_deref(), Some("mixed"));
        assert_eq!(&source[diagnostic.byte_range.clone()], "text 42");
    }
}

#[test]
fn diagnostics_do_not_attribute_presentation_children_to_the_previous_slide() {
    let source = "presentation {\n metadata { title \"x\" }\n slide id=\"first\" { text 42; }\n unexpected\n slide id=\"second\" { heading 42; }\n}";
    let document = PresentationDocument::from_source(source).unwrap();
    assert!(document.model().is_none());
    assert_eq!(document.diagnostics().len(), 3);
    let diagnostics = document.diagnostics();
    assert_eq!(diagnostics[0].slide_index, Some(0));
    assert_eq!(diagnostics[0].slide_id.as_deref(), Some("first"));
    assert_eq!(diagnostics[1].line, 4);
    assert_eq!(diagnostics[1].slide_index, None);
    assert_eq!(diagnostics[1].slide_id, None);
    assert_eq!(diagnostics[1].element_index, None);
    assert_eq!(diagnostics[2].slide_index, Some(1));
    assert_eq!(diagnostics[2].slide_id.as_deref(), Some("second"));
}

#[test]
fn syntax_diagnostics_use_kdl_newlines_and_unicode_columns() {
    for newline in [
        "\r\n", "\r", "\n", "\u{85}", "\u{2028}", "\u{2029}", "\u{c}",
    ] {
        let source = format!(
            "presentation {{{newline} metadata {{ title \"x\" }}{newline} slide {{ text \"日本語\\q\"; }}{newline}}}"
        );
        let document = PresentationDocument::from_source(&source).unwrap();
        assert!(document.model().is_none());
        assert!(!document.can_edit());
        assert!(!document.diagnostics().is_empty());
        for diagnostic in document.diagnostics() {
            assert_eq!(diagnostic.kind, DiagnosticKind::Syntax);
            assert_eq!(diagnostic.line, 3, "{diagnostic:?}");
            assert_eq!(diagnostic.source_line, " slide { text \"日本語\\q\"; }");
            let line_start = source.find(" slide").unwrap();
            assert_eq!(
                diagnostic.column,
                source[line_start..diagnostic.byte_range.start]
                    .chars()
                    .count()
                    + 1
            );
        }
    }
}

#[test]
fn syntax_diagnostics_identify_only_a_slide_proven_by_the_valid_prefix() {
    let prefix = "presentation { metadata { title \"x\"; }; slide id=\"first\" {};\n";
    for content in [
        "slide id=\"second\" { text \"bad\\q\"; }",
        "slide id=\"second\" { bad[; }",
        "slide id=\"second\" { columns { column { text \"bad\\q\"; } } }",
        "/* slide id=\"fake\" { */ slide id=\"second\" { text \"bad\\q\"; }",
        "/- slide id=\"ignored\" {}\n slide id=\"second\" { text \"bad\\q\"; }",
    ] {
        let document =
            PresentationDocument::from_source(&format!("{prefix}{content}\n}}")).unwrap();
        assert!(document.model().is_none());
        assert!(!document.diagnostics().is_empty());
        for diagnostic in document.diagnostics() {
            assert_eq!(diagnostic.slide_index, Some(1), "{diagnostic:?}");
            assert_eq!(diagnostic.slide_id.as_deref(), Some("second"));
        }
    }
    for content in [
        "bad[",
        "slide id=\"second\" {}; bad[",
        "/- slide id=\"ignored\" { bad[; }",
        "metadata { title \"slide id=\\\"fake\\\" {\\q\"; }",
        "/* slide id=\"fake\" { */ bad[",
    ] {
        let document =
            PresentationDocument::from_source(&format!("{prefix}{content}\n}}")).unwrap();
        assert!(!document.diagnostics().is_empty());
        for diagnostic in document.diagnostics() {
            assert_eq!(diagnostic.slide_index, None, "{diagnostic:?}");
            assert_eq!(diagnostic.slide_id, None);
        }
    }
}

#[test]
fn diagnostics_after_successive_edits_use_the_reopened_source() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("edited.kdl");
    std::fs::write(&path, "presentation {\r\n metadata { title \"x\" }\r\n slide id=\"edited\" { text \"before\"; }\r\n}").unwrap();
    let mut document = PresentationDocument::open(&path).unwrap();
    document.set_title("長いタイトル").unwrap();
    document.set_element_text(0, 0, "一行目\n二行目").unwrap();
    document.set_element_text(0, 0, "書き直した内容").unwrap();
    document.save().unwrap();
    let source = document
        .source()
        .replace("text \"書き直した内容\"", "text 42");
    std::fs::write(&path, &source).unwrap();
    let reopened = PresentationDocument::open(&path).unwrap();
    assert!(reopened.model().is_none());
    assert_eq!(reopened.diagnostics().len(), 1);
    let diagnostic = &reopened.diagnostics()[0];
    assert_eq!(diagnostic.file_name.as_deref(), Some("edited.kdl"));
    assert_eq!(diagnostic.line, 3);
    assert_eq!(diagnostic.column, 22);
    assert_eq!(diagnostic.source_line, " slide id=\"edited\" { text 42; }");
    assert_eq!(&source[diagnostic.byte_range.clone()], "text 42");
    assert_eq!(diagnostic.slide_index, Some(0));
    assert_eq!(diagnostic.slide_id.as_deref(), Some("edited"));
}
