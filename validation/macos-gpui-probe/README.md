# macOS / GPUI 先行検証プローブ

このプローブは Issue #18 の実機検証用コードである。
製品データは保存せず、保存操作の出力は `target/PROTOTYPE-title.txt` に置く。
編集表示と全画面発表は同じ 1280 × 720 の配置モデルを使う。
画面上部に表示する問いは「標準日本語 IME・共通配置・全画面復帰を GPUI で満たせるか」である。

## 検証環境

| 項目 | 値 |
| --- | --- |
| Mac | MacBook Air、Apple M3、arm64 |
| macOS | 26.5.2、Build 25F84 |
| Xcode | 26.5、Build 17F42 |
| Rust | 1.98.1 |
| GPUI | Zed commit `14dd03e89676e7fe74bc205001bfb32c8cfc3952` |
| IME | macOS 標準日本語入力 Kotoeri、RomajiTyping、Hiragana |
| 日本語フォント | Hiragino Sans |
| コードフォント | Menlo |
| 代替フォント | 個別指定なし。OS / GPUI のフォールバック先は未測定 |
| 内蔵ディスプレイ | 2560 × 1664 Retina、検証時の GPUI ウィンドウは 1470 × 922 pt、2x |
| 編集ウィンドウ指定サイズ | 小 1024 × 760 px、大 1440 × 960 px |

GPUI と GPUI platform は同一 Zed revision に固定している。
検証途中に BenQ GW2765 外部ディスプレイ 2560 × 1440 も検出された。
後の確認では内蔵ディスプレイだけが表示され、切替時刻は記録していない。
表示環境の時系列と未完了ケースは [`results.md`](results.md) に記録した。

## 起動

Xcode の Metal Toolchain と Rust 1.98.1 を利用できる macOS 環境で実行する。
このワークスペースでは Nix の `cc` が Xcode の SDK linker より先に解決されるため、次のように Xcode の compiler を指定する。

```sh
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ ~/.cargo/bin/cargo run --manifest-path validation/macos-gpui-probe/Cargo.toml
```

初回起動時は GPUI と依存クレートをビルドする。
入力欄には標準日本語 IME を使う。
`type-text` は IME を通らず文字列を直接挿入するため、IME 確認にはキーイベントを使う。

## 操作

| 操作 | 方法 |
| --- | --- |
| 画面サイズ | 「小さい編集表示」と「大きい編集表示」を選ぶ |
| 比率 1:1 | 「1 / 1:1」を選ぶ |
| 比率 1:2 | 「2 / 1:2」を選ぶ |
| はみ出し | 「3 / はみ出し」を選び、発表開始を試す |
| はみ出し修正 | 「はみ出し修正」を選ぶ |
| 最初から発表 | 「最初から発表」を選ぶ |
| 現在から発表 | 「現在から発表」を選ぶ |
| 発表中の次へ | Right、Down、Space、PageDown |
| 発表中の前へ | Left、Up、PageUp |
| 発表終了 | Escape |
| Undo / Redo | Cmd+Z / Cmd+Shift+Z |
| タイトル取消 | 「入力取消」を選ぶ |

発表中の前後移動は先頭と末尾で停止する。
空のタイトルと未確定 IME 文字列がある状態では保存と発表開始を止める。
はみ出し診断は Slide の余白内に要素枠が収まるかを検査する。
要素枠内に文字が収まらない場合の検出は、このプローブでは扱わない。

## 出力

起動するたびに `target/` 内の試験用タイトルと座標ログを初期化する。
`target/layout-coordinates.csv` は基本座標、配置倍率、画面倍率、論理座標、物理座標を記録する。
`target/ime-candidate-coordinates.csv` は IME に返した候補範囲の座標を記録する。
確定したタイトルは `target/PROTOTYPE-title.txt` に書く。

共有レイアウトと IME 入力処理の実装は `src/layout.rs` と `src/input.rs` にある。
手順別の観測結果と画面証跡は [`results.md`](results.md) を参照する。
用語と対象を先に固定した対応表は [`../../referent-table-macos-gpui-validation.md`](../../referent-table-macos-gpui-validation.md) である。
対応表の SHA-256 は `489a3f46cc4c4f108883c4bfc900b2cb0775572e93ffbfe1fcca4e268305bc5f` である。

## 参考資料

- [Apple 日本語入力ガイド](https://support.apple.com/ja-jp/guide/japanese-input-method/jpim10265/6.3/mac/26)
- [Apple 日本語入力の再変換](https://support.apple.com/guide/japanese-input-method/change-characters-you-entered-jpim10231/mac)
