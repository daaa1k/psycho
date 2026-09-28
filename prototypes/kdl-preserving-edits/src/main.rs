//! THROWAWAY: source-range editing experiment, not a production editor.
use kdl::{KdlDocument, KdlNode, KdlValue};
use serde_json::{Value, json};
use std::{collections::HashSet, ops::Range};

fn parse(s: &str) -> KdlDocument {
    s.parse().unwrap_or_else(|e| panic!("{e:?}\n{s}"))
}
fn node<'a>(d: &'a KdlDocument, path: &[usize]) -> &'a KdlNode {
    let n = &d.nodes()[path[0]];
    if path.len() == 1 {
        n
    } else {
        node(n.children().unwrap(), &path[1..])
    }
}
fn span(n: &KdlNode) -> Range<usize> {
    n.span().offset()..n.span().offset() + n.span().len()
}
fn semantic(n: &KdlNode) -> Value {
    json!({"name":n.name().value(), "entries":n.entries().iter().map(|e| json!([e.name().map(|n|n.value()), format!("{:?}",e.value())])).collect::<Vec<_>>(),"children":n.children().map(|d|d.nodes().iter().map(semantic).collect::<Vec<_>>())})
}
fn diagnostic(s: &str, n: &KdlNode, message: &str) -> Value {
    let r = span(n);
    json!({"message":message,"offset":r.start,"length":r.len(),"line":s[..r.start].bytes().filter(|b|*b==b'\n').count()+1})
}
fn validate(s: &str, d: &KdlDocument) -> Vec<Value> {
    fn walk(s: &str, n: &KdlNode, parent: &str, ids: &mut HashSet<String>, out: &mut Vec<Value>) {
        let name = n.name().value();
        let allowed = match parent {
            "" => vec!["presentation"],
            "presentation" => vec!["metadata", "slide"],
            "metadata" => vec!["title"],
            "slide" => vec!["heading", "text", "bullets", "code", "image", "columns"],
            "column" => vec!["heading", "text", "bullets", "code", "image"],
            "columns" => vec!["column"],
            "bullets" => vec!["item"],
            "image" => vec!["caption"],
            _ => vec![],
        };
        if !allowed.contains(&name) {
            out.push(diagnostic(s, n, "許可されないノード"));
        }
        let args = n
            .entries()
            .iter()
            .filter(|e| e.name().is_none())
            .collect::<Vec<_>>();
        let has_value = matches!(
            name,
            "title" | "heading" | "text" | "item" | "code" | "image" | "caption"
        );
        if args.len() != usize::from(has_value)
            || args.iter().any(|e| e.value().as_string().is_none())
        {
            out.push(diagnostic(s, n, "値の個数または型"));
        }
        if matches!(name, "title" | "image")
            && args
                .first()
                .and_then(|e| e.value().as_string())
                .is_some_and(|v| v.is_empty())
        {
            out.push(diagnostic(s, n, "空の必須値"));
        }
        if name == "image"
            && args
                .first()
                .and_then(|e| e.value().as_string())
                .is_some_and(|v| {
                    v.starts_with('/') || v.starts_with('\\') || v.as_bytes().get(1) == Some(&b':')
                })
        {
            out.push(diagnostic(s, n, "画像の絶対パス"));
        }
        let mut keys = std::collections::HashMap::new();
        for e in n.entries().iter().filter(|e| e.name().is_some()) {
            let key = e.name().unwrap().value();
            if let Some(first) = keys.insert(key, e.span().offset()) {
                out.push(json!({"message":"同名属性の重複","offset":e.span().offset(),"length":e.span().len(),"first_offset":first,"line":s[..e.span().offset()].bytes().filter(|b|*b==b'\n').count()+1}));
            }
            let good = match (name, key) {
                ("slide", "id") | ("code", "language") => e.value().as_string().is_some(),
                ("column", "width") => e
                    .value()
                    .as_integer()
                    .is_some_and(|v| (1..=99).contains(&v)),
                _ => false,
            };
            if !good {
                out.push(diagnostic(s, n, "属性名または型または範囲"));
            }
            if (name, key) == ("slide", "id") {
                if let Some(id) = e.value().as_string() {
                    if !ids.insert(id.into()) {
                        out.push(diagnostic(s, n, "重複した Slide ID"));
                    }
                }
            }
        }
        let children = n.children().map(|d| d.nodes()).unwrap_or(&[]);
        let names = children
            .iter()
            .map(|n| n.name().value())
            .collect::<Vec<_>>();
        let structure = match name {
            "presentation" => {
                names.first() == Some(&"metadata")
                    && names.iter().filter(|v| **v == "metadata").count() == 1
            }
            "metadata" => names == ["title"],
            "columns" => {
                names == ["column", "column"]
                    && children
                        .iter()
                        .filter_map(|n| n.get("width").and_then(|e| e.as_integer()))
                        .sum::<i128>()
                        == 100
            }
            "column" => keys.contains_key("width"),
            "image" => names.len() <= 1,
            "slide" | "bullets" => true,
            _ => children.is_empty(),
        };
        if !structure {
            out.push(diagnostic(s, n, "必須構造または列幅の合計"));
        }
        for c in children {
            walk(s, c, name, ids, out);
        }
    }
    let mut out = vec![];
    let mut ids = HashSet::new();
    if d.nodes().len() != 1 {
        out.push(json!({"message":"Presentation は 1 個","offset":0,"length":s.len(),"line":1}));
    }
    for n in d.nodes() {
        walk(s, n, "", &mut ids, &mut out);
    }
    out
}
#[derive(Clone)]
struct Edit {
    range: Range<usize>,
    text: String,
}
fn patch(s: &str, edits: &[Edit]) -> String {
    let mut edits = edits.to_vec();
    edits.sort_by_key(|e| e.range.start);
    let mut out = String::new();
    let mut cursor = 0;
    for e in &edits {
        assert!(e.range.start >= cursor);
        out.push_str(&s[cursor..e.range.start]);
        out.push_str(&e.text);
        cursor = e.range.end;
    }
    out.push_str(&s[cursor..]);
    out
}
fn unchanged(s: &str, out: &str, edits: &[Edit]) -> bool {
    let mut es = edits.to_vec();
    es.sort_by_key(|e| e.range.start);
    let (mut a, mut b) = (0, 0);
    for e in es {
        let len = e.range.start - a;
        if s.as_bytes()[a..e.range.start] != out.as_bytes()[b..b + len] {
            return false;
        }
        a = e.range.end;
        b += len + e.text.len();
    }
    s.as_bytes()[a..] == out.as_bytes()[b..]
}
fn value_edit(s: &str, path: &[usize], entry: usize, value: &str) -> Edit {
    let d = parse(s);
    let e = &node(&d, path).entries()[entry];
    let end = e.span().offset() + e.span().len();
    let repr = &e.format().unwrap().value_repr;
    let start = end - repr.len();
    assert_eq!(&s[start..end], repr);
    Edit {
        range: start..end,
        text: KdlValue::String(value.into()).to_string(),
    }
}
fn line_start(s: &str, pos: usize) -> usize {
    s[..pos].rfind('\n').map_or(0, |p| p + 1)
}
fn indent(s: &str, pos: usize) -> &str {
    let p = line_start(s, pos);
    let end = p + s[p..]
        .bytes()
        .take_while(|c| *c == b' ' || *c == b'\t')
        .count();
    &s[p..end]
}
// The parser's node span stops before its terminator. Comments after a semicolon
// can belong to the following node's leading field; classify by source position.
fn owned(s: &str, n: &KdlNode) -> Range<usize> {
    let mut r = span(n);
    let p = line_start(s, r.start);
    if s[p..r.start].bytes().all(|b| b == b' ' || b == b'\t') {
        r.start = p;
    }
    let b = s.as_bytes();
    let mut i = r.end;
    if b.get(i) == Some(&b';') {
        i += 1;
    }
    loop {
        while matches!(b.get(i), Some(b' ' | b'\t')) {
            i += 1;
        }
        if s[i..].starts_with("//") {
            i = s[i..].find('\n').map_or(s.len(), |p| i + p);
            break;
        }
        if s[i..].starts_with("/*") {
            let mut depth = 1;
            i += 2;
            while depth > 0 {
                if s[i..].starts_with("/*") {
                    depth += 1;
                    i += 2;
                } else if s[i..].starts_with("*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += s[i..].chars().next().unwrap().len_utf8();
                }
            }
            continue;
        }
        break;
    }
    if s[i..].starts_with("\r\n") {
        i += 2;
    } else if b.get(i) == Some(&b'\n') {
        i += 1;
    }
    r.end = i;
    r
}
// Reindent all lines uniformly, then require semantic equality after parsing.
// If a line has less indentation (e.g. a blank line), leave it intact.
fn reindent(text: &str, from: &str, to: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| match line.strip_prefix(from) {
            Some(rest) => format!("{to}{rest}"),
            None => line.into(),
        })
        .collect()
}
fn destination(s: &str, parent: &KdlNode) -> (usize, String, String) {
    let newline = if s.contains("\r\n") { "\r\n" } else { "\n" }.to_string();
    let children = parent.children().unwrap();
    let base = indent(s, parent.name().span().offset());
    let child_indent = children
        .nodes()
        .iter()
        .find_map(|n| {
            let p = n.name().span().offset();
            let i = indent(s, p);
            (i.len() > base.len()).then(|| i.to_string())
        })
        .unwrap_or_else(|| format!("{base}{}", if base.contains('\t') { "\t" } else { "    " }));
    let end = span(parent).end;
    let close = s[..end].rfind('}').unwrap();
    let line = line_start(s, close);
    let pos = if s[line..close].trim().is_empty() {
        line
    } else {
        close
    };
    (pos, child_indent, newline)
}
fn insert(s: &str, parent_path: &[usize], text: &str) -> Edit {
    let d = parse(s);
    let parent = node(&d, parent_path);
    let (pos, to, nl) = destination(s, parent);
    let prefix = if pos == line_start(s, pos) {
        String::new()
    } else {
        nl.clone()
    };
    let suffix = if pos == line_start(s, pos) {
        String::new()
    } else {
        indent(s, parent.name().span().offset()).to_string()
    };
    Edit {
        range: pos..pos,
        text: format!("{prefix}{to}{text}{nl}{suffix}"),
    }
}
// A small lexical scan for the experiment; KDL remains the syntax validator.
fn quoted_ranges(s: &str) -> Vec<Range<usize>> {
    let mut ranges = vec![];
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with("//") {
            i = s[i..].find('\n').map_or(s.len(), |p| i + p);
            continue;
        }
        if s[i..].starts_with("/*") {
            i += 2;
            let mut depth = 1;
            while depth > 0 {
                if s[i..].starts_with("/*") {
                    depth += 1;
                    i += 2;
                } else if s[i..].starts_with("*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += s[i..].chars().next().unwrap().len_utf8();
                }
            }
            continue;
        }
        let start = i;
        let hashes = s[i..].bytes().take_while(|b| *b == b'#').count();
        let quote = i + hashes;
        if s.as_bytes().get(quote) != Some(&b'"') {
            i += s[i..].chars().next().unwrap().len_utf8();
            continue;
        }
        let count = if s[quote..].starts_with("\"\"\"") {
            3
        } else {
            1
        };
        let close = format!("{}{}", "\"".repeat(count), "#".repeat(hashes));
        i = quote + count;
        while i < s.len() {
            if s[i..].starts_with(&close) {
                i += close.len();
                break;
            }
            if hashes == 0 && s.as_bytes()[i] == b'\\' {
                i += 1;
                if i < s.len() {
                    i += s[i..].chars().next().unwrap().len_utf8();
                }
            } else {
                i += s[i..].chars().next().unwrap().len_utf8();
            }
        }
        ranges.push(start..i);
    }
    ranges
}
fn reindent_protected(text: &str, from: &str, to: &str) -> String {
    let protected = quoted_ranges(text);
    let mut offset = 0;
    text.split_inclusive('\n')
        .map(|line| {
            let inside = protected.iter().any(|r| r.start < offset && offset < r.end);
            offset += line.len();
            if inside {
                line.to_string()
            } else {
                line.strip_prefix(from)
                    .map_or_else(|| line.to_string(), |rest| format!("{to}{rest}"))
            }
        })
        .collect()
}

fn move_candidate(
    s: &str,
    path: &[usize],
    parent_path: &[usize],
    require_equal: bool,
    protect_strings: bool,
) -> Vec<Edit> {
    let d = parse(s);
    let n = node(&d, path);
    let r = owned(s, n);
    let parent = node(&d, parent_path);
    let (pos, to, nl) = destination(s, parent);
    assert!(pos < r.start || pos >= r.end);
    let from = indent(s, n.name().span().offset());
    let original = &s[r.clone()];
    let shifted = if protect_strings {
        reindent_protected(original, from, &to)
    } else {
        reindent(original, from, &to)
    };
    // Inline nodes lack a leading indentation prefix: put one on the first line.
    let moved = if original.starts_with(from) && !from.is_empty() {
        shifted
    } else {
        format!("{to}{}", shifted.trim_start_matches([' ', '\t']))
    };
    let moved = if moved.ends_with('\n') {
        moved
    } else {
        format!("{moved}{nl}")
    };
    let prefix = if pos == line_start(s, pos) {
        String::new()
    } else {
        nl.clone()
    };
    let suffix = if pos == line_start(s, pos) {
        String::new()
    } else {
        indent(s, parent.name().span().offset()).to_string()
    };
    // The moved element is reparsed in isolation, independently of the document.
    let md = parse(&moved);
    assert_eq!(md.nodes().len(), 1);
    if require_equal {
        assert_eq!(
            semantic(n),
            semantic(&md.nodes()[0]),
            "moving must preserve all values and children"
        );
    }
    vec![
        Edit {
            range: r,
            text: String::new(),
        },
        Edit {
            range: pos..pos,
            text: format!("{prefix}{moved}{suffix}"),
        },
    ]
}
fn move_node(s: &str, path: &[usize], parent_path: &[usize]) -> Vec<Edit> {
    move_candidate(s, path, parent_path, true, false)
}
struct Case {
    title: String,
    description: String,
    initial: String,
    current: String,
    steps: Vec<Value>,
}
impl Case {
    fn new(title: &str, description: &str, s: &str) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            initial: s.into(),
            current: s.into(),
            steps: vec![],
        }
    }
    fn step(
        &mut self,
        label: &str,
        edits: Vec<Edit>,
        expect_valid: bool,
        check: impl FnOnce(&str, &KdlDocument),
    ) {
        let before = self.current.clone();
        let after = patch(&before, &edits);
        let d = parse(&after);
        let diags = validate(&after, &d);
        assert_eq!(
            diags.is_empty(),
            expect_valid,
            "{} / {label}: {diags:?}",
            self.title
        );
        assert!(unchanged(&before, &after, &edits));
        check(&after, &d);
        self.steps.push(json!({"label":label,"before":before,"after":after,"edits":edits.iter().map(|e|json!({"start":e.range.start,"end":e.range.end,"text":e.text})).collect::<Vec<_>>(),"unchanged":true,"syntax":true,"schema":diags.is_empty(),"diagnostics":diags,"structure":d.nodes().iter().map(semantic).collect::<Vec<_>>() }));
        self.current = after;
    }
    fn json(self) -> Value {
        json!({"title":self.title,"description":self.description,"initial":self.initial,"steps":self.steps})
    }
}
fn main() {
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).unwrap();
    let mut cases = vec![];
    let s = "\u{feff}presentation {\r\n\tmetadata { title \"例\" }\r\n\tslide { text #\"日本\"# } \r\n\tslide { image \"assets/a.png\" }\r\n}";
    let mut c = Case::new(
        "無編集保存と値の変更",
        "BOM、CRLF、末尾改行なし、閉じ括弧の後の空白を保持する。",
        s,
    );
    c.step("無編集で保存", vec![], true, |a, _| assert_eq!(a, s));
    let e = value_edit(&c.current, &[0, 1, 0], 0, "日本語の新しい本文\n次の行");
    c.step("本文を変更", vec![e], true, |a, d| {
        assert!(a.contains("} \r\n"));
        assert_eq!(
            node(d, &[0, 1, 0]).entries()[0].value().as_string(),
            Some("日本語の新しい本文\n次の行")
        );
    });
    let e = value_edit(&c.current, &[0, 2, 0], 0, "assets/新しい画像.png");
    c.step("画像パスを変更", vec![e], true, |_, d| {
        assert_eq!(
            node(d, &[0, 2, 0]).entries()[0].value().as_string(),
            Some("assets/新しい画像.png")
        )
    });
    cases.push(c.json());

    let s = "presentation {\n    metadata { title \"コメント\" }\n    slide {\n        // 独立コメント: この場所に残す\n        image \"a.png\" {\n            // 内部コメント: 画像に伴う\n            caption \"図\"\n        }; // 末尾コメント: 画像に伴う\n        text \"残す\"; text \"同じ行\"; // 二つ目に伴う\n        columns {\n            column width=50 {\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n";
    let mut c = Case::new(
        "コメントを伴う移動と削除",
        "独立コメントを元の位置に残し、内部と末尾コメントだけを画像に伴わせる。",
        s,
    );
    let e = move_node(&c.current, &[0, 1, 0], &[0, 1, 3, 0]);
    c.step("画像を左列へ移動", e, true, |a, d| {
        assert!(a.contains("// 独立コメント: この場所に残す\n        text"));
        assert_eq!(node(d, &[0, 1, 2, 0, 0]).name().value(), "image");
        assert!(a.contains("}; // 末尾コメント: 画像に伴う"));
    });
    let d = parse(&c.current);
    let r = owned(&c.current, node(&d, &[0, 1, 2, 0, 0]));
    c.step(
        "画像を削除",
        vec![Edit {
            range: r,
            text: String::new(),
        }],
        true,
        |a, _| {
            assert!(a.contains("独立コメント"));
            assert!(!a.contains("内部コメント"));
            assert!(!a.contains("末尾コメント"));
        },
    );
    let e = move_node(&c.current, &[0, 1, 1], &[0, 1, 2, 1]);
    c.step(
        "同じ行の二つ目の本文を右列へ移動",
        e,
        true,
        |a, d| {
            assert!(a.contains("text \"残す\"; "));
            assert_eq!(
                node(d, &[0, 1, 1, 1, 0]).entries()[0].value().as_string(),
                Some("同じ行")
            );
            assert!(a.contains("text \"同じ行\"; // 二つ目に伴う"));
        },
    );
    cases.push(c.json());

    for (title, repr) in [
        ("通常文字列", r#""引用: \"x\" と改行\n日本語""#),
        ("生文字列", r##"#"C:\path\file "引用""#"##),
        (
            "複数行文字列",
            "\"\"\"\n            fn main() {\n                println!(\"日本\");\n            }\n\n            \"\"\"",
        ),
        (
            "生の複数行文字列",
            "#\"\"\"\n            \\n はそのまま\n                二行目\n            \"\"\"#",
        ),
    ] {
        let s = format!(
            "presentation {{\n    metadata {{ title \"移動\" }}\n    slide {{\n        code language=\"rust\" {repr}\n        columns {{\n            column width=60 {{\n            }}\n            column width=40 {{\n            }}\n        }}\n    }}\n}}\n"
        );
        let before = semantic(node(&parse(&s), &[0, 1, 0]));
        let mut c = Case::new(
            title,
            "列の内外へ移動し、再解析した文字列内容と構造を比較する。",
            &s,
        );
        let e = move_node(&c.current, &[0, 1, 0], &[0, 1, 1, 0]);
        let expected = before.clone();
        c.step("コードを左列へ移動", e, true, |_, d| {
            assert_eq!(semantic(node(d, &[0, 1, 0, 0, 0])), expected)
        });
        let e = move_node(&c.current, &[0, 1, 0, 0, 0], &[0, 1]);
        c.step("コードを列の外へ戻す", e, true, |_, d| {
            assert_eq!(semantic(node(d, &[0, 1, 1])), before)
        });
        cases.push(c.json());
    }
    let s = "presentation {\r\n\tmetadata { title \"追加\" }\r\n\tslide {\r\n\t\ttext \"既存\"\r\n\t}\r\n\tslide {}\r\n}\r\n";
    let mut c = Case::new(
        "追加と周囲の書式",
        "既存行は変えず、タブと CRLF に合わせる。空の親では親の書式から補う。",
        s,
    );
    let e = insert(&c.current, &[0, 1], "text \"追加\"");
    c.step("末尾に本文を追加", vec![e], true, |a, d| {
        assert!(a.contains("\t\ttext \"追加\"\r\n"));
        assert_eq!(node(d, &[0, 1]).children().unwrap().nodes().len(), 2);
    });
    let e = insert(&c.current, &[0, 2], "heading \"空の親へ\"");
    c.step("空の Slide に追加", vec![e], true, |a, d| {
        assert!(a.contains("\t\theading"));
        assert_eq!(node(d, &[0, 2, 0]).name().value(), "heading");
    });
    let d = parse(&c.current);
    let n = node(&d, &[0, 1, 0]);
    let pos = line_start(&c.current, n.name().span().offset());
    c.step(
        "先頭に本文を追加",
        vec![Edit {
            range: pos..pos,
            text: "\t\ttext \"先頭\"\r\n".into(),
        }],
        true,
        |_, d| {
            assert_eq!(
                node(d, &[0, 1, 0]).entries()[0].value().as_string(),
                Some("先頭")
            )
        },
    );
    cases.push(c.json());

    let s = "presentation {\n    metadata { title \"診断\" }\n    slide { code language /* 名 */ = /* 値 */ \"rust\" /-language=\"ignored\" \"日本\" }\n}\n";
    let mut c = Case::new(
        "連続編集後の診断位置",
        "値の範囲を毎回取り直す。重複属性は不正入力の診断実験として最後に挿入し、保存不可を確認する。",
        s,
    );
    let e = value_edit(&c.current, &[0, 1, 0], 0, "javascript");
    c.step("言語を変更", vec![e], true, |a, _| {
        assert!(a.contains("/* 名 */ = /* 値 */ javascript /-language=\"ignored\""))
    });
    let e = value_edit(&c.current, &[0, 1, 0], 1, "日本語を長くする\n二行目");
    c.step("本文を長く変更", vec![e], true, |_, d| {
        assert_eq!(
            node(d, &[0, 1, 0]).entries()[1].value().as_string(),
            Some("日本語を長くする\n二行目")
        )
    });
    let e = insert(&c.current, &[0, 1], "text \"追加\"");
    c.step("本文を追加", vec![e], true, |_, _| {});
    let d = parse(&c.current);
    let n = node(&d, &[0, 1, 0]);
    let pos = n.entries()[1].span().offset() + n.entries()[1].span().len();
    c.step(
        "診断実験: language を重複させる",
        vec![Edit {
            range: pos..pos,
            text: " language=\"python\"".into(),
        }],
        false,
        |a, d| {
            let diag = validate(a, d);
            assert_eq!(diag.len(), 1);
            let p = diag[0]["offset"].as_u64().unwrap() as usize;
            let first = diag[0]["first_offset"].as_u64().unwrap() as usize;
            assert!(a[p..].starts_with("language=\"python\""));
            assert!(a[first..].starts_with("language /* 名 */"));
        },
    );
    cases.push(c.json());
    let s = "presentation {\n    metadata { title \"不均一なインデント\" }\n    slide {\n        code \"\"\"\n        significant spaces\nzero indent\n\"\"\"\n        columns {\n            column width=50 {\n            }\n            column width=50 {\n            }\n        }\n    }\n}\n";
    let mut c = Case::new(
        "一律のインデント変更の反例",
        "閉じ引用符が行頭にある有効な複数行文字列。行頭の空白を一律に置換すると、文字列内容が変わる。再解析で差を検出し、移動を確定しない。",
        s,
    );
    let edits = move_candidate(s, &[0, 1, 0], &[0, 1, 1, 0], false, false);
    let candidate = patch(s, &edits);
    let cd = parse(&candidate);
    assert!(validate(&candidate, &cd).is_empty());
    let original_value = semantic(node(&parse(s), &[0, 1, 0]));
    let candidate_value = semantic(node(&cd, &[0, 1, 0, 0, 0]));
    assert_ne!(original_value, candidate_value);
    c.step(
        "左列への移動を試す（内容が変わるため保留）",
        vec![],
        true,
        |a, _| assert_eq!(a, s),
    );
    c.steps.last_mut().unwrap()["rejected_candidate"] = json!({"source":candidate,"before":original_value,"after":candidate_value,"reason":"構文と Schema が有効でも文字列内容が変わるため、この移動を確定できない"});
    let edits = move_candidate(&c.current, &[0, 1, 0], &[0, 1, 1, 0], true, true);
    c.step(
        "文字列の表記を保持して左列へ移動",
        edits,
        true,
        |_, d| {
            assert_eq!(semantic(node(d, &[0, 1, 0, 0, 0])), original_value);
        },
    );
    let edits = move_candidate(&c.current, &[0, 1, 0, 0, 0], &[0, 1], true, true);
    c.step(
        "文字列の表記を保持して列の外へ戻す",
        edits,
        true,
        |_, d| {
            assert_eq!(semantic(node(d, &[0, 1, 1])), original_value);
        },
    );
    cases.push(c.json());
    let s = "presentation {\r\n\tmetadata { title \"字句の区別\" }\r\n\tslide {\r\n\t\tcode /* 外側 \" /* 内側 */ */ /-#\"\"\"\r\n\t\t無効化した文字列\r\n\"\"\"# #\"\"\"\r\n\t\t実際の文字列\r\n\"\"\"#; // 伴うコメント\r\n\t\tcolumns {\r\n\t\t\tcolumn width=50 {\r\n\t\t\t}\r\n\t\t\tcolumn width=50 {\r\n\t\t\t}\r\n\t\t}\r\n\t}\r\n}\r\n";
    let mut c = Case::new(
        "文字列とコメントを区別した移動",
        "CRLF、タブ、入れ子のブロックコメント、slashdash された生の複数行文字列を含む。引用された表記をバイト単位で保持する。",
        s,
    );
    let d = parse(s);
    let r = owned(s, node(&d, &[0, 1, 0]));
    let original = &s[r];
    let tokens = quoted_ranges(original)
        .into_iter()
        .map(|r| original[r].to_string())
        .collect::<Vec<_>>();
    let original_semantic = semantic(node(&d, &[0, 1, 0]));
    let edits = move_candidate(s, &[0, 1, 0], &[0, 1, 1, 0], true, true);
    c.step(
        "生の複数行文字列を左列へ移動",
        edits,
        true,
        |a, d| {
            let r = owned(a, node(d, &[0, 1, 0, 0, 0]));
            let text = &a[r];
            assert_eq!(
                tokens,
                quoted_ranges(text)
                    .into_iter()
                    .map(|r| text[r].to_string())
                    .collect::<Vec<_>>()
            );
            assert_eq!(original_semantic, semantic(node(d, &[0, 1, 0, 0, 0])));
            assert!(text.ends_with("; // 伴うコメント\r\n"));
        },
    );
    let edits = move_candidate(&c.current, &[0, 1, 0, 0, 0], &[0, 1], true, true);
    c.step("列の外へ戻す", edits, true, |a, d| {
        let r = owned(a, node(d, &[0, 1, 1]));
        let text = &a[r];
        assert_eq!(
            tokens,
            quoted_ranges(text)
                .into_iter()
                .map(|r| text[r].to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(original_semantic, semantic(node(d, &[0, 1, 1])));
    });
    cases.push(c.json());
    let result =
        json!({"parser":"kdl 6.7.1 / 8dac0428c75e09d22078b3a1703c70bb0912392d","cases":cases});
    std::fs::write(
        "results.json",
        serde_json::to_string_pretty(&result).unwrap(),
    )
    .unwrap();
    let template = std::fs::read_to_string("viewer.html").unwrap();
    std::fs::write(
        "demo.html",
        template.replace(
            "/*RESULTS*/null",
            &result.to_string().replace('<', "\\u003c"),
        ),
    )
    .unwrap();
    println!(
        "{} scenarios / {} operations: PASS",
        result["cases"].as_array().unwrap().len(),
        result["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["steps"].as_array().unwrap().len())
            .sum::<usize>()
    );
}
