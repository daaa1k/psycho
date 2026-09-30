# macOS / GPUI 先行検証プローブ

このプローブは Issue #18 の実機検証用コードである。
製品データは保存せず、保存操作の出力は `target/PROTOTYPE-title.txt` に置く。
編集表示と全画面発表は同じ 1280 × 720 の配置モデルを使う。
画面上部に表示する問いは「標準日本語 IME、共通配置、全画面復帰を GPUI で満たせるか」である。

## 検証環境

| 項目 | 値 |
| --- | --- |
| Mac | MacBook Air、Apple M3、arm64 |
| macOS | 26.5.2、Build 25F84 |
| Xcode | 26.5、Build 17F42 |
| Rust | 1.98.1 |
| GPUI / GPUI platform | Zed commit `14dd03e89676e7fe74bc205001bfb32c8cfc3952` |
| ディスプレイ | 内蔵 Liquid Retina、2560 × 1664 Retina。今回の `system_profiler` 取得では内蔵画面のみ |
| GPUI window scale | 1 |
| 小さい編集表示 | viewport 1024 × 760、ウィンドウ 1024 × 792 |
| 大きい編集表示 | viewport 1440 × 960、ウィンドウ 1440 × 992 |
| 全画面 | viewport とウィンドウが 1920 × 1200 |
| 日本語フォント | Hiragino Sans |
| コードフォント | Menlo |
| 代替フォント | 個別指定なし。OS / GPUI のフォールバック先は未測定 |
| 日本語 IME | macOS 標準 Kotoeri（ローマ字入力）で実機確認。候補確定と Undo / Redo は修正前に確認し、範囲計算修正後の再試験待ち |

GPUI と GPUI platform は同一 Zed revision に固定している。
AeroSpace を有効にしていた初回の寸法ログには、小さい編集表示の途中で 1472 × 965 や 1890 × 1169 が記録された。
AeroSpace を停止して Orca を再起動した後は、上表の小さい編集表示、大きい編集表示、全画面の寸法で証跡を取り直した。
初回の記録は [AeroSpace 有効時のウィンドウ寸法ログ](evidence/layout-window-dimensions-aerospace-enabled.csv)、取り直した配置ログは [AeroSpace 無効時の配置座標ログ](evidence/layout-coordinate-log-aerospace-disabled.csv) にある。
判定と未完了ケースは [検証結果](results.md) に記録した。

## 起動

Xcode の Metal Toolchain と Rust 1.98.1 を利用できる macOS 環境で実行する。
このワークスペースでは Nix の `cc` が Xcode の SDK linker より先に解決されるため、次のように Xcode の compiler を指定する。

```sh
env PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH CC=/usr/bin/clang CXX=/usr/bin/clang++ ~/.cargo/bin/cargo run --manifest-path validation/macos-gpui-probe/Cargo.toml
```

初回起動時は GPUI と依存クレートをビルドする。
入力欄には macOS 標準日本語 IME を使う。
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

## 出力と録画

起動するたびに `target/` 内の試験用タイトルと座標ログを初期化する。
`target/layout-coordinates.csv` はウィンドウと viewport の寸法、基本座標、配置倍率、画面倍率、論理座標、物理座標を記録する。
`target/ime-candidate-coordinates.csv` は IME に返した候補範囲の座標を記録する。
確定したタイトルは `target/PROTOTYPE-title.txt` に書く。
編集サイズを選ぶと maximized 状態を解除してから指定寸法へ変更する。

macOS のアプリウィンドウだけを録画する場合は、全画面表示に切り替えた後に次の形式で実行する。

```sh
screencapture -v -l <window-id> -V 15 -x validation/macos-gpui-probe/evidence/presentation.mov
```

全画面への切替前に録画を始めると、ウィンドウ枠に合わせて映像が切り取られる。
現在の全画面録画と手順別の画面証跡は [検証結果](results.md) を参照する。
IME の画面証跡はあるが、IME 操作の録画はまだ作成していない。

共有レイアウトと IME 入力処理の実装は `src/layout.rs` と `src/input.rs` にある。
用語と対象を先に固定した対応表は [referent table](../../referent-table-macos-gpui-validation.md) である。
対応表の SHA-256 は `489a3f46cc4c4f108883c4bfc900b2cb0775572e93ffbfe1fcca4e268305bc5f` である。

## 参考資料

- [Apple 日本語入力ガイド](https://support.apple.com/ja-jp/guide/japanese-input-method/jpim10265/6.3/mac/26)
- [Apple 日本語入力の再変換](https://support.apple.com/guide/japanese-input-method/change-characters-you-entered-jpim10231/mac)
