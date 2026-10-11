use psycho::PresentationDocument;
use std::fs;

fn assert_saved_history(
    doc: &mut PresentationDocument,
    path: &std::path::Path,
    before: &str,
    after: &str,
) {
    assert_eq!(doc.source(), after);
    doc.save().unwrap();
    assert_eq!(fs::read(path).unwrap(), after.as_bytes());
    assert_eq!(PresentationDocument::open(path).unwrap().source(), after);
    assert!(doc.undo());
    assert_eq!(doc.source(), before);
    assert!(doc.redo());
    assert_eq!(doc.source(), after);
}

#[test]
fn moved_quoted_escaped_newlines_keep_original_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed.kdl");
    let quoted = "\"one\\\r\n  two\\\n\tthree\"";
    let source = format!(
        "presentation {{\n\tmetadata {{ title \"mixed\" }}\n\tslide {{\n\t\tcode {quoted}\n\t\tcolumns {{\r\n\t\t\tcolumn width=50 {{\r\n\t\t\t}}\r\n\t\t\tcolumn width=50 {{}}\r\n\t\t}}\n\t}}\n}}\n"
    );
    fs::write(&path, &source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    let expected = format!(
        "presentation {{\n\tmetadata {{ title \"mixed\" }}\n\tslide {{\n\t\tcolumns {{\r\n\t\t\tcolumn width=50 {{\r\n\t\t\t\tcode {quoted}\r\n\t\t\t}}\r\n\t\t\tcolumn width=50 {{}}\r\n\t\t}}\n\t}}\n}}\n"
    );
    assert_saved_history(&mut doc, &path, &source, &expected);
}

#[test]
fn tab_indented_comment_and_brace_move_out_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tabs.kdl");
    let source = "presentation {\n\tmetadata { title \"tabs\" }\n\tslide {\n\t\timage \"a.png\" {\n\t\t\t/* comment \" /* nested */ */\n\t\t\tcaption \"caption\"\n\t\t}\n\t\tcolumns {\n\t\t\tcolumn width=50 {}\n\t\t\tcolumn width=50 {}\n\t\t}\n\t}\n}\n";
    fs::write(&path, source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    let nested = "presentation {\n\tmetadata { title \"tabs\" }\n\tslide {\n\t\tcolumns {\n\t\t\tcolumn width=50 {\n\t\t\t\timage \"a.png\" {\n\t\t\t\t\t/* comment \" /* nested */ */\n\t\t\t\t\tcaption \"caption\"\n\t\t\t\t}\n\t\t\t}\n\t\t\tcolumn width=50 {}\n\t\t}\n\t}\n}\n";
    assert_saved_history(&mut doc, &path, source, nested);
    doc.move_column_element_to_slide_at(0, 0, 0, 0, 0).unwrap();
    let back = source.replacen("column width=50 {}", "column width=50 {\n\t\t\t}", 1);
    assert_saved_history(&mut doc, &path, nested, &back);
}

fn assert_move_after_comment_newline(newline: &str) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("comment-newline.kdl");
    let quoted = "#\"\"\"\r\n                hello\r\n\"\"\"#";
    let source = format!(
        "presentation {{\n    metadata {{ title \"newlines\" }}\n    slide {{\n        image \"a.png\" {{\n            // 日本語 note{newline}            caption {quoted}\n        }}\n        columns {{\n            column width=50 {{}}\n            column width=50 {{}}\n        }}\n    }}\n}}\n"
    );
    fs::write(&path, &source).unwrap();
    let mut doc = PresentationDocument::open(&path).unwrap();
    assert!(doc.can_edit(), "{newline:?}: {:?}", doc.diagnostics());
    doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
    assert!(
        doc.source().contains(quoted),
        "quoted bytes changed for {newline:?}: {:?}",
        doc.source()
    );
    let outside_newline = "\n";
    let caption_indent = "                    ";
    let expected = format!(
        "presentation {{\n    metadata {{ title \"newlines\" }}\n    slide {{\n        columns {{\n            column width=50 {{\n                image \"a.png\" {{\n                    // 日本語 note{outside_newline}{caption_indent}caption {quoted}\n                }}\n            }}\n            column width=50 {{}}\n        }}\n    }}\n}}\n"
    );
    assert_saved_history(&mut doc, &path, &source, &expected);
}

macro_rules! comment_newline_test {
    ($name:ident, $newline:literal) => {
        #[test]
        fn $name() {
            assert_move_after_comment_newline($newline);
        }
    };
}

comment_newline_test!(moved_quoted_after_comment_nel, "\u{85}");
comment_newline_test!(moved_quoted_after_comment_form_feed, "\u{c}");
comment_newline_test!(moved_quoted_after_comment_vertical_tab, "\u{b}");
comment_newline_test!(moved_quoted_after_comment_line_separator, "\u{2028}");
comment_newline_test!(moved_quoted_after_comment_paragraph_separator, "\u{2029}");
comment_newline_test!(moved_quoted_after_comment_cr, "\r");
comment_newline_test!(moved_quoted_after_comment_lf, "\n");
comment_newline_test!(moved_quoted_after_comment_crlf, "\r\n");

#[test]
fn all_kdl_newlines_bound_owned_comment_and_header() {
    for newline in [
        "\n", "\r\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("boundaries.kdl");
        let prefix = "presentation {\n    metadata { title \"newlines\" }\n    slide {\n        // standalone\n";
        let suffix = "    }\n}\n";
        let source = format!(
            "{prefix}        text \"a\" // owned{newline}        text \"b\"{newline}{suffix}"
        );
        fs::write(&path, &source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        assert!(doc.can_edit(), "{newline:?}: {:?}", doc.diagnostics());
        doc.remove_element(0, 0)
            .unwrap_or_else(|error| panic!("delete {newline:?}: {error:?}"));
        let expected = format!("{prefix}        text \"b\"{newline}{suffix}");
        assert_saved_history(&mut doc, &path, &source, &expected);
        doc.undo();
        doc.move_element(0, 0, 1)
            .unwrap_or_else(|error| panic!("move {newline:?}: {error:?}"));
        let expected = format!(
            "{prefix}        text \"b\"{newline}        text \"a\" // owned{newline}{suffix}"
        );
        assert_saved_history(&mut doc, &path, &source, &expected);
        let source = format!("{prefix}        code \"body\"{newline}{suffix}");
        fs::write(&path, &source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        doc.set_element_field(0, 0, psycho::ElementField::CodeLanguage, "rust")
            .unwrap_or_else(|error| panic!("header {newline:?}: {error:?}"));
        let expected = format!("{prefix}        code \"body\" language=\"rust\"{newline}{suffix}");
        assert_saved_history(&mut doc, &path, &source, &expected);
    }
}

#[test]
fn all_kdl_newlines_allow_child_insertion_and_column_roundtrip() {
    for newline in [
        "\n", "\r\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("children.kdl");
        let quoted = "#\"\"\"\n    quoted\n\"\"\"#";
        let source = format!(
            "presentation {{{newline}    metadata {{ title \"newlines\" }}{newline}    slide {{{newline}        image \"a.png\"{newline}        columns {{{newline}            column width=50 {{ // close follows{newline}            }}{newline}            column width=50 {{}}{newline}        }}{newline}    }}{newline}}}{newline}"
        );
        fs::write(&path, &source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        assert!(doc.can_edit(), "{newline:?}: {:?}", doc.diagnostics());
        doc.set_element_field(0, 0, psycho::ElementField::Caption, "caption")
            .unwrap();
        let expected = source.replacen(
            "image \"a.png\"",
            &format!(
                "image \"a.png\" {{{newline}            caption \"caption\"{newline}        }}"
            ),
            1,
        );
        assert_saved_history(&mut doc, &path, &source, &expected);
        doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
        assert!(doc.source().contains(&format!("                image \"a.png\" {{{newline}                    caption \"caption\"{newline}                }}")), "{newline:?}: {:?}", doc.source());
        doc.move_column_element_to_slide_at(0, 0, 0, 0, 0).unwrap();
        assert_eq!(doc.source(), expected);
        let source = source.replace("image \"a.png\"", &format!("code {quoted}"));
        fs::write(&path, &source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        doc.move_element_to_column_at(0, 0, 1, 0, 0).unwrap();
        assert!(doc.source().contains(quoted));
        doc.move_column_element_to_slide_at(0, 0, 0, 0, 0).unwrap();
        assert_eq!(doc.source(), source);
    }
}

#[test]
fn destination_newline_style_excludes_quoted_token_newlines() {
    for quoted_newline in [
        "\r", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quoted-style.kdl");
        let quoted = format!("#\"\"\"{quoted_newline}body{quoted_newline}\"\"\"#");
        let source = format!(
            "presentation {{\n    metadata {{ title \"style\" }}\n    slide {{\n        text \"move\"\n        columns {{\n            column width=50 {{ code {quoted}; }}\n            column width=50 {{}}\n        }}\n    }}\n}}\n"
        );
        fs::write(&path, &source).unwrap();
        let mut doc = PresentationDocument::open(&path).unwrap();
        assert!(
            doc.can_edit(),
            "{quoted_newline:?}: {:?}",
            doc.diagnostics()
        );
        doc.move_element_to_column_at(0, 0, 1, 0, 1).unwrap();
        let expected = format!(
            "presentation {{\n    metadata {{ title \"style\" }}\n    slide {{\n        columns {{\n            column width=50 {{ code {quoted}; \n                text \"move\"\n            }}\n            column width=50 {{}}\n        }}\n    }}\n}}\n"
        );
        assert_saved_history(&mut doc, &path, &source, &expected);
    }
}
