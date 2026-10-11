# 編集画面 #49 の実機検証

GitHub Issues [#49](https://github.com/daaa1k/psycho/issues/49) と親仕様 [#48](https://github.com/daaa1k/psycho/issues/48) に対する公開 GUI 操作の記録です。macOS 26.5.2、単一接続画面で確認しました。スクリーンショットは Orca の製品ウィンドウ取得です。KDL は `target/` 内の新規ファイルだけを編集しました。

## 結果と対象リビジョン

| 確認 | 結果 | ソースと証跡 |
| --- | --- | --- |
| ライトとダーク、1024×700・1280×860・1440×960の編集領域 | PASS | `c2a7f77`、`evidence/light/` と `evidence/dark/` の `run.json` と各寸法の画像 |
| 両ペイン開閉、Fit・100%、キャンバススクロール、メニュー幅変更 | PASS | 同上、`observations.json`、表示操作の前後で KDL・dirty・文書履歴・draft の不変を照合 |
| ペインと倍率変更前後の共通文字配置 | PASS | 同上、`glyph-comparison.json` と `presentation-glyphs.csv`。20 glyph の基準座標を倍率とキャンバス原点で正規化。3寸法と4種類以上の倍率で位置差0.001px未満、glyph ID と並びを維持 |
| macOS Edit、入力と文書の Undo/Redo、Cut・Copy・Paste | PASS | `27e83f4`、`evidence/edit-menu/edit-menu-observations.json`。新しい popup 履歴は次行で追加確認 |
| 入力後に表示 popup を開き、ツールバー・macOS Edit・Cmd-Z で文書 Undo | PASS | `9041f7b`、`evidence/popup-after/`。`evidence/popup-before/` は `c2a7f77` の反応しない不具合の再現 |
| 欠落画像のパスを未確定のまま修正して「発表」を選ぶ | PASS | `9041f7b`、`evidence/regressions/asset/`。修正を確定して発表でき、発表だけでは実ファイルを保存しない |
| ダークで構文・Schema 診断を選び、読み取り専用ソースを表示 | PASS | `9041f7b`、`evidence/regressions/syntax/` と `schema/`。発表は無効のままでファイルを変更しない |
| UI 文字と選択ソースのコントラスト | PASS | `contrast.json`。役割別32組のパレット比の最小5.33。実ダーク診断画像の通常文字・警告・キャレットと選択背景も4.5以上 |
| native「開く」で6枚の代表例を開き、本文編集・保存・Undo/Redo・発表・復帰 | PASS | `9041f7b`、`evidence/regressions/representative/`。画像・コード・箇条書き・2列を含む `examples/build-time.kdl` のコピーを使用 |
| Navigator と Inspector の境界ドラッグ | PASS | 同上、`navigator-width-drag.json` と `inspector-width-drag.json`。公開マウス操作の実幅変化を確認。既存 `slow_drag.swift` を使用 |
| popup Escape の入力復帰、Tab で Inspector の入力へ移動して本文編集・Cmd-S | PASS | `ef4a9c9`、`evidence/focus/`。入力 handle の Tab 登録漏れを修正 |
| 標準日本語 IME の変換中に native 発表メニューを実行、未保存で発表・復帰・明示保存 | PASS | `9041f7b`、`evidence/ime/` と `ime.log`。元の ABC 入力ソースと有効状態へ復元 |
| 既存公開操作テスト | PASS | `ef4a9c9`、`evidence/cargo-test.log`。`cargo test --all-targets --features gui` の114件 |

`ef4a9c9` は `9041f7b` から入力 focus handle の `.tab_stop(true)` だけを変更しています。`9041f7b` は `c2a7f77` から popup の操作選択時のフォーカス復帰だけを変更しています。配置・色・保存・発表を変更していないため、既存の実機結果は上表の実行リビジョンを保って記録しています。証跡や README だけを追加したコミットを、実行済みのリビジョンとして扱いません。

Tab は操作と常時表示の Inspector 入力を辿ります。キャンバス上の inline 入力は既存仕様で blur 後に静的テキストへ戻るため、そこへ Tab だけで入り直すことは確認対象に含めていません。popup Escape で開く前の inline 入力へ戻るケースは別に確認しています。複数行入力中の Tab はタブ文字を挿入します。

## VoiceOver

VoiceOver の読み上げは未証明です。実起動、welcome の終了、`NSWorkspace.shared.isVoiceOverEnabled=true` は確認しました。修飾キー設定は実 UI で「⌃⌥ または ⇪」でした。native キーによる35回の移動では読み上げ文字列を取得できませんでした。操作名・ボタンの役割・無効状態の実際の読み上げを確認できていないため、#49 のこの基準は残件です。AX ツリーやプロセスの存在を読み上げの証明には使っていません。

scripted 試行は AppleScript 制御チェックボックスのクリックが `window_not_focused` で失敗し、後始末の UI 読取も `runtime_unavailable`（Orca runtime closed the connection before responding）で失敗しました。`last phrase` 取得には到達していません。`evidence/voiceover/voiceover.json` に具体的なエラーと未確認項目を記録しています。

復旧後、再起動したタスク所有の Utility の実 UI で AppleScript 制御が元値0、welcome 設定が元値1と読み返しました。ネイティブ AX でも制御0を確認し、Workspace getter は false でした。Utility と検証用アプリを終了し、VoiceOver・Quickstart・検証用アプリの残存プロセスがないことを確認しました。最終 scripted 試行では caption 設定の変更に到達していません。それ以前の native caption キー試行は対称に切り替えましたが、表示状態の読戻しはできていません。IME は wrapper の記録どおり ABC と元の有効状態へ復元しました。

日本語 IME の再変換に関する既存 [#31](https://github.com/daaa1k/psycho/issues/31) と、2画面 Retina の描画像比較 [#46](https://github.com/daaa1k/psycho/issues/46) はこの変更で解決済みとして扱いません。

## 再実行

GUI ドライバーを同時に動かさず、毎回新しい `target/` ディレクトリを指定します。Orca の Accessibility・Screen Recording 接続と Xcode が必要です。テストと build の双方に実リビジョンを渡します。

```sh
revision=$(git rev-parse HEAD)
PSYCHO_BUILD_REVISION="$revision" cargo test --all-targets --features gui
PSYCHO_BUILD_REVISION="$revision" cargo build --features gui
python3 validation/editor-shell/acceptance.py --output target/editor-shell-light-new --appearance light
python3 validation/editor-shell/acceptance.py --output target/editor-shell-dark-new --appearance dark
python3 validation/editor-shell/popup_history.py --output target/editor-shell-popup-new
python3 validation/editor-shell/regressions.py --output target/editor-shell-regressions-new
python3 validation/editor-shell/focus.py --output target/editor-shell-focus-new
python3 validation/editor-shell/compare_glyphs.py target/editor-shell-dark-new/presentation-glyphs.csv --output target/editor-shell-dark-new/glyph-comparison.json
python3 validation/editor-shell/contrast.py --evidence target/editor-shell-regressions-new --output target/editor-shell-regressions-new/contrast.json
xcrun swift validation/mvp-acceptance/with_japanese_ime.swift /usr/bin/python3 validation/mvp-acceptance/presentation_ime_acceptance.py --output target/editor-shell-ime-new
```

`contrast.py` はソースのパレット値を読むとともに、実画像の支配色を照合します。画面取得の色変換を含むため、ソースとの一致判定にはRGB各成分12以内の許容幅を記録しています。選択背景に対する実画像の通常文字・警告・キャレットの比も別に計算します。アンチエイリアス境界の全ピクセル一致は判定しません。

各 `run.json` と `editor-state.json` がビルドリビジョン、入力と文書履歴、保存した KDL を記録します。`observations.json` と各段階の JSON が AX・実幅・倍率・入力状態を記録します。PNG が画面の確認資料です。再実行用 Swift バイナリ、OCR バイナリ、動画は保存していません。

接続断で cleanup が中断した VoiceOver 試行スクリプトは再実行用として保存していません。失敗の観測と復旧結果を保存し、native 状態読取・復旧用 Swift ヘルパーだけを残しています。
