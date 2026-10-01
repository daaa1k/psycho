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
| 代替フォント | 個別指定なし。CoreText では Menlo の日本語を `HiraginoSans-W3`、絵文字を `AppleColorEmoji` に解決。全グリフのID・位置・ラスタをCoreText参照と照合 |
| 日本語 IME | macOS 標準 Kotoeri（ローマ字入力）で候補確定、再変換、Undo / Redo と候補位置の追従を実機確認。未確定中の保存と発表は変換を確定して進んだ。確定直後を100回と描画修正後10回検査して欠落・重複なし。過去の未再現の観測はIssue #31へ移した |

GPUI と GPUI platform は同一 Zed revision に固定している。
AeroSpace を有効にしていた初回の寸法ログには、小さい編集表示の途中で 1472 × 965 や 1890 × 1169 が記録された。
AeroSpace を停止して Orca を再起動した後は、上表の小さい編集表示、大きい編集表示、全画面の寸法で証跡を取り直した。
初回の記録は [AeroSpace 有効時のウィンドウ寸法ログ](evidence/layout-window-dimensions-aerospace-enabled.csv)、取り直した配置ログは [AeroSpace 無効時の配置座標ログ](evidence/layout-coordinate-log-aerospace-disabled.csv) にある。
各ケースの判定は[検証結果](results.md)に記録した。

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
空のタイトルでは保存と発表開始を止める。
未確定 IME 文字列がある状態では変換を確定してから保存または発表を開始する。
「入力取消」「無効入力」「タイトル修正」でも IME の変換セッションを終了する。
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
IME 操作の録画は [修正版の IME 入力録画](evidence/ime-fixed-region-live.mov) にある。
画像枠とコード枠の物理ピクセル境界は次のコマンドで再測定する。
`sips` と `ffmpeg` を使用し、[測定値](evidence/layout-raster-measurements.csv)を再生成する。

```sh
/usr/bin/python3 validation/macos-gpui-probe/measure_raster.py
```

文字を含む要素の先頭画素が 3 サイズで配置とともに移動するかは次のコマンドで測る。
[測定値](evidence/layout-text-anchor-consistency.csv)はサイズ間の一貫性を示し、全グリフの絶対位置の検証には使わない。

```sh
/usr/bin/python3 validation/macos-gpui-probe/measure_text_anchors.py
```

CoreText のフォント解決先は次のコマンドで再取得する。

```sh
/usr/bin/swift validation/macos-gpui-probe/inspect_font_fallback.swift
```

共有レイアウトと文字描画は `src/layout.rs` と `src/text.rs`、IME入力処理は `src/input.rs` にある。
用語と対象を先に固定した対応表は [referent table](../../referent-table-macos-gpui-validation.md) である。
対応表の SHA-256 は `489a3f46cc4c4f108883c4bfc900b2cb0775572e93ffbfe1fcca4e268305bc5f` である。

## 参考資料

- [Apple 日本語入力ガイド](https://support.apple.com/ja-jp/guide/japanese-input-method/jpim10265/6.3/mac/26)
- [Apple 日本語入力の再変換](https://support.apple.com/guide/japanese-input-method/change-characters-you-entered-jpim10231/mac)

## 確定直後のIME反復検査

`repeat_ime.py` はKotoeriの実キー入力を反復し、各Return直後の画面OCR、描画文字列、その間のIME更新コールバック、保存値を検査する。10回ごとに再変換も試す。二重表示の既存画像も同じOCRで検出できる。入力ソースの切替はスクリプトに含めない。標準Kotoeriのローマ字入力を選び、試験後は開始時の入力ソースへ戻す。

```sh
/usr/bin/swiftc validation/macos-gpui-probe/recognize_heading.swift -o /tmp/psycho-recognize-heading
mkdir -p validation/macos-gpui-probe/target/ime-trace
env PSYCHO_IME_TRACE_DIR="$PWD/validation/macos-gpui-probe/target/ime-trace" PSYCHO_GLYPH_TRACE="$PWD/validation/macos-gpui-probe/target/shared-glyphs.csv" validation/macos-gpui-probe/target/debug/macos-gpui-probe
```

別の端末で `orca computer list-windows --app macos-gpui-probe --json` を実行し、PIDとwindow IDを取得する。小さい編集表示のSlide 1で、次を実行する。

```sh
/usr/bin/python3 validation/macos-gpui-probe/repeat_ime.py --app pid:<PID> --window-id <WINDOW_ID> --ocr /tmp/psycho-recognize-heading --iterations 100
```

`target/ime-loop/` に全確定直後画像と `checks.csv` が残る。`target/ime-trace/ime-events.tsv` はイベント名、文字列のUTF-8 hex、UTF-8選択範囲、未確定範囲、挿入文字列のUTF-8 hex、指定置換範囲をタブ区切りで記録する。`ime-painted.txt` は最後に入力欄へ描画した文字列である。ログは追記されるので、別セッションの記録は実行前に退避する。

100回の実測は[検査値](evidence/ime-repeat-100-checks.csv)、[全確定直後画像](evidence/ime-repeat-100-snapshots.zip)、[100回の検査と直前の試走10回のコールバック列](evidence/ime-repeat-100-events.tsv.gz)に保存した。再現しない二重表示の観測は[Issue #31](https://github.com/daaa1k/psycho/issues/31)で追跡する。

## 全グリフの独立測定

静的なSlide文字は `src/text.rs` が基準サイズで字形と改行を決め、原点とグリフ位置を一緒に拡縮する。コードのタブは描画時だけ4文字のタブストップへ展開し、折り返さない。保存文字列は変更しない。行高やコード余白を表示サイズごとに丸めない。

`layout-reference.json` は比較する文字列、共通の改行、基準座標、固定フォントを独立した参照値として定義する。`render_reference.swift` はGPUIを呼ばず、CoreTextで基準サイズの全グリフを求め、同率拡縮した座標と参照画像を出力する。実画像から位置を推定して参照値を補正する処理はない。

```sh
/usr/bin/python3 -m venv /tmp/psycho-measure-env
/tmp/psycho-measure-env/bin/pip install numpy==2.0.2 Pillow==11.3.0
/usr/bin/swiftc validation/macos-gpui-probe/render_reference.swift -o /tmp/psycho-render-reference
/tmp/psycho-render-reference validation/macos-gpui-probe/layout-reference.json validation/macos-gpui-probe/target/layout-reference
```

IME検査と同じ環境変数でプローブを起動し、上のPIDとwindow IDを指定して6画面を取得する。AeroSpaceなどの配置制御は試験中だけ無効にし、実行前の状態へ戻す。

```sh
/usr/bin/python3 validation/macos-gpui-probe/capture_layout.py --app pid:<PID> --window-id <WINDOW_ID>
/tmp/psycho-measure-env/bin/python validation/macos-gpui-probe/measure_glyphs.py
```

`capture_layout.py` は画面寸法を読み戻してから保存する。ネイティブのサイズ変更中に操作ツールのフォーカス検査が失敗した場合も、クリックを繰り返す前に実際の状態を取得する。

測定は1:1と1:2の全12要素、空白を除く491グリフを3サイズ、計1,473件比較する。空白の送り量は後続グリフの座標で検査する。グリフID、数と順序が一致し、GPUIに渡した原点をGPUIの規則どおりxは1/4px、yは1pxへ丸めた座標が、CoreTextの参照座標から各軸1px以内でなければ不合格になる。

画像側では、全キャンバスと各グリフ領域の濃い画素が相手画像の薄い画素から各軸1px以内にあるかを双方向で検査する。通常文字の濃い画素はRGB最大値100未満、Captionは140未満、薄い画素は220未満とする。その間の濃淡差はアンチエイリアスの許容差として判定から除く。文字の欠け、余分な描画、意図しない改行が残れば不合格になる。

保存済み証跡だけでも再測定できる。

```sh
/tmp/psycho-measure-env/bin/python validation/macos-gpui-probe/measure_glyphs.py --reference validation/macos-gpui-probe/evidence/shared-layout
```

修正途中でコード枠と内側余白を別々に丸めた版は、次のコマンドで51グリフが不合格になり、終了コード1を返す。最大誤差は1.056582px。完成版は全1,473件が合格し、最大誤差は0.494689pxである。

```sh
/tmp/psycho-measure-env/bin/python validation/macos-gpui-probe/measure_glyphs.py --reference validation/macos-gpui-probe/evidence/shared-layout --images validation/macos-gpui-probe/evidence/intermediate-rounding --mode large --slide 2 --output validation/macos-gpui-probe/target/intermediate-rounding-check.csv
```
