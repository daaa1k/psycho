# Slide Navigator #50 の実機検証

対象は [Issue #50](https://github.com/daaa1k/psycho/issues/50) と [親仕様 #48](https://github.com/daaa1k/psycho/issues/48) です。実装は `4379632` です。編集、発表、サムネイルが同じ `render_slide_body` を明示した倍率で呼びます。文書の選択位置は `current_slide` が保持し、Navigator はフォーカスとスクロールだけを保持します。

## 確認結果

| 対象 | 結果 | 証跡 |
| --- | --- | --- |
| 全対象 GUI テスト114件 | PASS | `evidence/cargo-test.log` |
| 明暗の100枚、クリック・上下・Return・末尾への native スクロール | PASS | `evidence/dark/` と `evidence/light/` の `run.json`、`observations.json`、`visible-ranges.json` |
| 本文編集、Undo/Redo、外部正常・無効再読み込み、画像再読み込み | PASS | 同上。最初の無効文書と競合中の draft 保持は別の新規ウィンドウで確認 |
| 2列内の全要素と文字配置、画像の2:1比率 | PASS | 各 `glyph-comparison.json` と `image-ratios.json`。85 glyph の最大基準座標差0.000222px。可視範囲は1要求あたり最大6行 |
| 空スライドの白背景、欠落画像と Caption、readonly サムネイル | PASS | 各 `blank-slide.png`、`missing-placeholder.png`、`readonly-thumbnail.json` |
| 明暗の1024×700、1280×860、1440×960 の先頭と末尾、上下キーの往復 | PASS | `evidence/layout-dark/observations.json` と `evidence/layout-light/observations.json`、各寸法の PNG |
| Navigator の開閉、幅変更、文書と履歴の不変 | PASS | 同上 |
| 長い日本語見出しの省略表示と、実ホバーによる全文ツールチップ | PASS | `evidence/layout-dark/full-heading-tooltip.png`。OCR と実画像を照合 |
| VoiceOver の番号、全文見出し、選択状態の実発話 | 未確認 | `evidence/voiceover/verification.json` |

`acceptance.py` は100枚の資料を生成します。見出し、本文、箇条書き、コード、画像、各要素を含む2列、空スライド、欠落画像を配置します。公開 GUI 操作で選択、上下キー、Return、手動スクロール、本文編集、Undo/Redo、外部変更、同じパスの画像差し替えを実行します。キャンバスとサムネイルの文字位置は glyph ID と基準座標を照合し、画像比率は取得した実画像の支配色から測ります。色変換と選択枠の混入を避けるため、色を分類して同じ支配色の連続した帯を比較します。

`layout.py` は3寸法と Navigator の開閉・幅変更を確認します。実際にポインターを2枚目へ移動し、全文ツールチップを OCR で照合します。表示操作前後で KDL、dirty、Undo/Redo、draft を照合します。100枚という資料枚数と可視範囲の記録は仮想化の確認であり、操作時間の測定ではありません。

`compare_readonly.py` は列内の見出しを選択する前後の実画像から、サムネイル本体だけを切り出します。編集枠を追加してもサムネイルの画像が変わらないことを照合します。枠の外にある Navigator の選択・フォーカス表示は比較範囲に含めません。

## VoiceOver の残件

実起動後の `NSWorkspace.shared.isVoiceOverEnabled=true`、Utility の AppleScript 制御設定、native AX の ListBoxOption と選択属性までは確認しました。実発話の取得は `content of last phrase` が AppleEvent エラー `-10000` で失敗しました。AX ツリー、役割、プロセスの存在を読み上げ成功の証明には使っていません。Issue #50 は実発話の受け入れ基準を残しています。

AppleScript 制御設定は元の0へ戻しました。welcome 設定は最終読戻しで0だったため、元の1へ復元しました。`quit` は VoiceOver プロセスを停止しても Workspace の有効値を true に残したため、検証用ウィンドウへ native Cmd-F5 を送り、実起動から停止へ進めました。最後の Workspace getter は false でした。VoiceOver、Quickstart、Utility と検証用アプリの残存プロセスがないことも確認しました。`evidence/voiceover/cleanup.json` と `final-native-settings.json` が読戻しの記録です。

## 再実行

Orca の Accessibility・画面収録接続、Xcode、Pillow が必要です。GUI ドライバーを同時に動かさず、新しい `target/` ディレクトリを指定します。

```sh
revision=$(git rev-parse HEAD)
env CC=/usr/bin/clang CXX=/usr/bin/clang++ CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang PSYCHO_BUILD_REVISION="$revision" cargo test --all-targets --features gui
env CC=/usr/bin/clang CXX=/usr/bin/clang++ CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang PSYCHO_BUILD_REVISION="$revision" cargo build --features gui
python3 -m venv target/navigator-env-new
target/navigator-env-new/bin/pip install Pillow
target/navigator-env-new/bin/python validation/navigator/acceptance.py --output target/navigator-dark-new --appearance dark --revision "$revision"
target/navigator-env-new/bin/python validation/navigator/acceptance.py --output target/navigator-light-new --appearance light --revision "$revision"
target/navigator-env-new/bin/python validation/navigator/layout.py --output target/navigator-layout-dark-new --appearance dark --revision "$revision"
target/navigator-env-new/bin/python validation/navigator/layout.py --output target/navigator-layout-light-new --appearance light --revision "$revision"
target/navigator-env-new/bin/python validation/navigator/compare_readonly.py target/navigator-dark-new
target/navigator-env-new/bin/python validation/navigator/compare_readonly.py target/navigator-light-new
```

Nix の `cc` が先に選ばれる環境では `-liconv` のリンクに失敗します。C コンパイラに加え Rust linker も Xcode Clang に指定します。検証専用の環境変数は任意指定で、文書と表示設定を保存する機能は追加していません。
