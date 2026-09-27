# Rust KDL 実装による保存差分の保証の調査

調査日: 2026-09-27。
対象: [既存 KDL 実装で保存差分の保証を満たせるか調べる](https://github.com/daaa1k/psycho/issues/11)。
要求は [GUI 編集と KDL 保存の保証を決める](https://github.com/daaa1k/psycho/issues/5#issuecomment-5855363165) と [Presentation KDL v0.1 の表現範囲を決める](https://github.com/daaa1k/psycho/issues/3#issuecomment-5853733009) の決定による。
本文の前に [対応表](../../referent-table-kdl-preserving-edits.md) を作成した（SHA256: `66f2448ebb1fdd7a7e2bd7a544c59cf8ed11c421a35f75e4887d933efbbdbfa5`）。

## 結論と対象のバージョン

`kdl` は構文解析と元の属性の検証に使える候補だが、**6.7.1 の文書全体を `Display` で保存する方法は、未編集部分のバイト保持を満たさない**。
今回の実行で、子を持つノードの閉じ括弧の後に空白があると空白と改行が消える反例を確認した。
値だけの変更にも表記の明示的な更新が必要であり、コメントの所属と移動先のインデントは PSYCHO 側で補う必要がある。
ライブラリ採用と保存方式の確定前に、原文を保持して必要な範囲だけ差し替える方式を含めた追加の試作を行うことを提案する。
この方式の完全な適合性は本調査では実証していない。

調査対象は `kdl` **6.7.1**、公式 tag `v6.7.1` の commit **`8dac0428c75e09d22078b3a1703c70bb0912392d`** に固定した。
Rust 1.95.0、`default-features = false` と `features = ["span"]` で実行した。
同版の MSRV は 1.95、既定 feature は `span` と `serde` である。
KDL 2.0 のみを受理する前提なので、KDL 1.0 を再試行する `v1-fallback` は有効にしない。[Cargo.toml](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/Cargo.toml)、[解析 API](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/document.rs)

公式の実装一覧は Rust の書式保持実装として `kdl-rs` を挙げ、`knus` は KDL 1 のみと表示している。
`knus` 3.4.0 とその前身 `knuffel` の公開 API は型へのデコードを中心としており、今回必要な書式保持の編集 API を選ぶ理由にはならない。
このため `kdl-rs` を実行検証の対象とした。
Rust 実装全体の網羅調査ではない。[公式実装一覧](https://kdl.dev/#implementations)、[knus 3.4.0](https://docs.rs/knus/3.4.0/knus/)、[knuffel 公開 API](https://docs.rs/knuffel/latest/knuffel/)

## 既存 API で得られる情報と補う範囲

| 要求 | 得られる情報と API | 補う範囲 |
| --- | --- | --- |
| 未編集部分のバイト保持 | 元の値表記、識別子表記、コメント、空白、終端の保持用フィールド | 実行で全体再出力の反例あり。原文を保持し、変更外の範囲を元のバイト列から出力する方法などを試す |
| 値だけの変更 | `KdlEntry::value`、`set_value`、`format_mut().value_repr` | 意味上の値と出力表記を両方更新する。全体再出力の反例は別途避ける |
| 要素の追加 | `nodes_mut()`、`KdlNode::new`、書式フィールド | 周囲の改行形式とインデントを判定し、挿入位置の終端を調整する |
| コメントを伴う移動と削除 | `nodes_mut()` が返す `Vec`、`leading`、`before_terminator`、`terminator`、子文書 | 位置に基づいて独立コメントを残し、内部と同じ行の末尾コメントを追従させる |
| 文字列内容を変えないインデント調整 | 意味上の `KdlValue::String` と元の `value_repr` の両方 | 構文を区別して調整し、出力を再解析して内容が等しいか確認する |
| 同名属性の重複と位置 | `entries()` の全件順序、各 entry と識別子の `span()` | 名前による単一値取得だけで検証しない。編集後は位置を再計算する |

これらの API と書式フィールドは [entry.rs](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/entry.rs)、[node.rs](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/node.rs)、[document.rs](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/document.rs) で確認した。
上表の「補う範囲」は、公開 API と以下の実行結果からの判断であり、製品の内部型の決定ではない。

## 未編集文書の再出力で失われる内容

固定した commit の `tests/test_cases/input` にある 320 ファイルを、改行の正規化なしで読み込み、解析できたものを `to_string()` と比較した。
**230 件が一致、5 件が不一致、85 件が解析拒否**だった。
この入力集合は不正な KDL の試験入力も含むため、85 件を不適合数とは数えない。
期待される構文受理との比較はこの検証に含めていない。
上流の適合試験は改行を正規化して意味上の期待値と比較しており、その成功だけでは今回のバイト保持を確認できない。[上流の適合試験](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/tests/compliance.rs)

| 上流 fixture | 入力 → 出力（改行は `\n`） |
| --- | --- |
| `space_in_node_type.kdl` | `( type)node\n` → `(type)node\n` |
| `comment_in_node_type.kdl` | `(type/*hey*/)node\n` → `(type)node\n` |
| `comment_after_node_type.kdl` | `(type)/*hey*/node\n` → `(type)node\n` |
| `space_after_node_type.kdl` | `(type) node\n` → `(type)node\n` |
| `space_after_node.kdl` | `node1 {\n    child\n} \nnode2 {\n    child\n}\n` → `node1 {\n    child\n}node2 {\n    child\n}\n` |

型注釈を使わない Presentation にも最後の反例が影響する。
次の入力は KDL として解析できるが、出力は 2 個の `slide` が `}    slide` で連結され、再解析が失敗した。

```kdl
presentation {
    metadata { title "x" }
    slide { text "a" }␠
    slide { text "b" }
}
```

上記の `␠` はスペース 1 個の可視表記であり、実入力では U+0020 に置き換える。
`KdlNode::stringify` はノード型注釈の内部と直後の書式フィールドを出力せず、`base_node` は子ブロックの直後の空白を消費する前に終端を確認する。
この実装と観測した欠落は対応する。[ノードの出力処理](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/node.rs#L827-L879)、[ノードの解析処理](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/v2_parser.rs#L399-L443)

独自の 10 入力では、BOM、CRLF、日本語、raw string、複数行文字列、コメント、slashdash、末尾改行なしを含む未編集の再出力が一致した。
これは各入力に限定した確認であり、すべての KDL 表記の保証ではない。

## 値だけの変更と元の位置

`set_value("new")` は保持された `value_repr` を更新しない。
そのため、解析済み entry に `set_value` だけを呼ぶと、意味上の値は変わっても出力は古い値のままだった。
`value_repr` を新しい `KdlValue::String` の出力へ置き換えると、BOM と CRLF とコメントを含む検証入力で値の表記だけが変わった。
`clear_format()` は空白やコメントも消すので、値だけの変更の代わりには使えない。[値の更新と出力](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/entry.rs#L70-L82)、[書式の消去](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/entry.rs#L142-L150)

`entries()` は同名属性を元の順序のまま保持した。
`n key=1 key=2 /-key=3\n` では有効な 2 件を取得し、位置はそれぞれ `2..7` と `8..13` だった。
slashdash された属性は有効な entry から除かれるが、元のテキストは書式側に残る。
名前による `get` は最後の値を返すため、Presentation Schema の重複検証には entry の列挙が必要である。[entry の取得](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/node.rs#L118-L186)、[slashdash と entry の解析](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/v2_parser.rs#L539-L613)

位置は読み込んだ原文に対するものであり、編集後の更新は保証されない。
`text "日本" key=1 key=2\n` では各 entry の範囲は `5..13`、`14..19`、`20..25` であり、UTF-8 のバイト位置として原文を切り出せた。
entry の範囲は属性名や型注釈を含み、値トークンだけの独立した公開 span はない。
値だけの差し替えには `value_repr` と構文情報から対象範囲を求める処理を検証する必要がある。
構文エラーには原文と診断の span、理由、ラベルが提供される。[span の制約](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/entry.rs#L84-L101)、[診断](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/error.rs)

## コメントの所属とインデント

パーサーは独立した前置コメントを次のノードの `leading` に含める。
`// standalone\ntext "old" // tail\ntext next\n` の最初のノードを `Vec::remove` で削除すると、独立コメントも消えた。
末尾の `// tail` はそのノードの `terminator` に格納され、削除に追従する。
一方、`n; // same-line\nnext\n` では同じ行のコメントが次のノードの `leading` に入り、`n` の削除後にも残った。
したがって、書式フィールドの所有者をそのまま PSYCHO のコメント所属と見なせない。[ノード間の解析](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/v2_parser.rs#L281-L342)

移動でも削除と同じ所属調整が必要になる。
追加時には元ノードを変更せず、新規ノードのインデントと終端だけを周辺に合わせる処理が要る。
`autoformat_config` はノード名、entry、子文書を再帰的に整形し、終端なども書き換えるため、移動に伴う行頭空白だけの調整 API としては使えない。[自動整形処理](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/node.rs#L277-L329)

複数行文字列は閉じ引用符の行に合わせて共通インデントが取り除かれる。
行頭の空白の一括置換は、文字列の内容と閉じ引用符の関係を変えるおそれがある。[KDL 2.0 の文字列仕様](https://kdl.dev/spec/#name-multi-line-strings)
今回、単純な複数行文字列の本文と閉じ引用符を同じ量だけ深くする操作では、再解析した文字列内容の一致を確認した。
raw の複数行文字列、空行、タブ混在、エスケープによる空白除去、slashdash 内の文字列まで含めた一般的な移動処理は未検証である。

## 実装前の追加の試作

次の問いを一つの試作チケットで扱うことを提案する。
「原文を保持する保存処理と `kdl` 6.7.1 の解析情報を組み合わせ、既に合意した保存差分を実現できるか。必要な補完処理はどこまでか」。
以下を自動比較し、実際の変更差分を人間が確認してから方式を決める。

1. 未編集保存の入力と出力が完全一致する。今回の 5 反例、BOM、LF/CRLF、末尾改行なしを含める。
2. 文字列と画像パスの変更で、引用形式の変更を含む値の範囲以外が完全一致する。型注釈、属性の `=` 前後のコメント、日本語、slashdash を含める。
3. 先頭、末尾、空の親、異なる親間で追加と移動と削除ができる。独立コメントは元の位置に残り、内部と同じ行の末尾コメントは要素に追従する。セミコロン終端も含める。
4. 入れ子の深さを変えて移動した後、複数行文字列と raw 文字列の意味上の内容が等しく、無関係な既存行のバイト列が等しい。
5. 出力を再解析して構文と Presentation Schema が有効なままであり、連続編集後も値の対象範囲と診断位置が正しい。重複属性を両方の元位置とともに検出できる。

この調査ではライブラリへの修正、製品実装、ファイル保存の原子性、外部変更との競合制御は検証していない。
`kdl` の数値は `i128` と `f64` であり、任意精度の KDL 数値全体を保持する意味上のモデルではないが、v0.1 の列幅 1〜99 の表現範囲は満たす。[値の型](https://github.com/kdl-org/kdl-rs/blob/8dac0428c75e09d22078b3a1703c70bb0912392d/src/value.rs)
既存の仕様を変更する判断は本調査では行っていない。

## 再現用の調査コード

以下は製品コードではなく、今回 `/tmp` で実行した確認コードである。
固定 commit を clone し、`/tmp/psycho-kdl-research` に配置する。
別の一時 Cargo プロジェクトに以下の依存を置き、後続のコードを `src/main.rs` として `cargo run` する。

```toml
[package]
name = "psycho-kdl-probe"
version = "0.0.0"
edition = "2024"

[dependencies]
kdl = { path = "/tmp/psycho-kdl-research", default-features = false, features = ["span"] }
```

実行時の依存解決は `winnow 0.7.15`、`miette 7.6.0`、`num-traits 0.2.19`、`memchr 2.8.3`、`cfg-if 1.0.5`、`unicode-width 0.1.14`、`autocfg 1.5.1` だった。

```rust
use kdl::{KdlDocument, KdlValue};
fn main() {
    let mut ok = 0;
    let mut fail = 0;
    let mut rejected = 0;
    for file in std::fs::read_dir("/tmp/psycho-kdl-research/tests/test_cases/input").unwrap() {
        let path = file.unwrap().path();
        let s = std::fs::read_to_string(&path).unwrap();
        match s.parse::<KdlDocument>() {
            Ok(d) => {
                if d.to_string() == s {
                    ok += 1;
                } else {
                    fail += 1;
                    println!(
                        "ROUNDTRIP FAIL {:?}: input={:?} output={:?}",
                        path.file_name(),
                        s,
                        d.to_string()
                    );
                }
            }
            Err(_) => rejected += 1,
        }
    }
    println!("corpus roundtrip accepted_equal={ok} accepted_different={fail} rejected={rejected}");

    let ps = "presentation {\n    metadata { title \"x\" }\n    slide { text \"a\" } \n    slide { text \"b\" }\n}\n";
    let pd: KdlDocument = ps.parse().unwrap();
    println!("presentation output {:?}", pd.to_string());
    println!(
        "presentation output reparses {}",
        pd.to_string().parse::<KdlDocument>().is_ok()
    );
    let u: KdlDocument = "text \"日本\" key=1 key=2\n".parse().unwrap();
    for e in u.nodes()[0].entries() {
        println!(
            "utf8 entry {:?}: {}..{}",
            e.value(),
            e.span().offset(),
            e.span().offset() + e.span().len()
        );
    }
    let cases = [
        "\u{feff}// ヘッダ\r\ntext  #\"old\"# // tail\r\n",
        "// standalone\ntext \"old\" // tail\ntext next\n",
        "n key=1 key=2 /-key=3\n",
        "n; // same-line\nnext\n",
        "p {\n  code \"\"\"\n    a\n      b\n    \"\"\"\n}\n",
        "n /-\"ignored\" \"kept\"\n",
        "n\r\n",
        "n",
        "// only\r\n",
        "n 1_000 \\\n  #true\n",
    ];
    for (i, s) in cases.iter().enumerate() {
        let d: KdlDocument = s.parse().unwrap();
        assert_eq!(d.to_string(), *s);
        println!("roundtrip {i}: PASS");
    }
    let s = cases[0];
    let mut d: KdlDocument = s.parse().unwrap();
    let e = &mut d.nodes_mut()[0].entries_mut()[0];
    e.set_value("new");
    assert_eq!(d.to_string(), s);
    println!("set_value retains old repr: PASS");
    let e = &mut d.nodes_mut()[0].entries_mut()[0];
    e.format_mut().unwrap().value_repr = KdlValue::String("new".into()).to_string();
    assert_eq!(d.to_string(), s.replace("#\"old\"#", "new"));
    println!("value_repr only replacement: PASS");
    let d: KdlDocument = cases[2].parse().unwrap();
    for e in d.nodes()[0].entries() {
        println!(
            "duplicate {:?} span {}..{}",
            e.value(),
            e.span().offset(),
            e.span().offset() + e.span().len()
        );
    }
    assert_eq!(d.nodes()[0].entries().len(), 2);
    for s in [cases[1], cases[3]] {
        let mut d: KdlDocument = s.parse().unwrap();
        println!(
            "node formats: {:?}",
            d.nodes().iter().map(|n| n.format()).collect::<Vec<_>>()
        );
        d.nodes_mut().remove(0);
        println!("remove first: {:?}", d.to_string());
    }
    let mut d: KdlDocument = cases[4].parse().unwrap();
    let e = &mut d.nodes_mut()[0]
        .children_mut()
        .as_mut()
        .unwrap()
        .nodes_mut()[0]
        .entries_mut()[0];
    let before = e.value().clone();
    e.format_mut().unwrap().value_repr = e
        .format()
        .unwrap()
        .value_repr
        .replace("\n    ", "\n        ");
    let output = d.to_string();
    let reparsed: KdlDocument = output.parse().unwrap();
    assert_eq!(
        &before,
        reparsed.nodes()[0].children().unwrap().nodes()[0].entries()[0].value()
    );
    println!("multiline coordinated indent: PASS");
}

```
