# 編集画面の通し受け入れ #56

[Issue #56](https://github.com/daaa1k/psycho/issues/56) と [親仕様 #48](https://github.com/daaa1k/psycho/issues/48) の受け入れ記録。同じ製品バイナリーを実ウィンドウで操作した記録。総合判定は未受け入れ。必須の日本語IME変換中の Cmd+S で既知の [#77](https://github.com/daaa1k/psycho/issues/77) が再現した。

## 実行対象

開始時の `main` は `4526fa5`。#54 の [PR #76](https://github.com/daaa1k/psycho/pull/76) は `fix/verify-native-input` へマージされており、`main` に Overview が存在しなかった。`64fa4b9` で取り込み、`06eb12e` で初回不正文書の診断呼び出しを #55 の引数へ合わせた。今回の製品バイナリーは `06eb12e` を埋め込んでビルドした。後続の検証スクリプトと文書のコミットは、製品の実行リビジョンと区別する。

macOS 26.5.2、build 25F84。接続画面は内蔵1枚。AppKit の画面座標は1920×1200、利用可能領域は1920×1170、backing scale は1。編集領域1024×700、1280×860、1440×960に対応するウィンドウの高さは、タイトルバーを含め732、892、992となる。

[ビルド記録](evidence/build-provenance.json.gz)、[Rustテスト](evidence/integration-repair-tests.log.gz)、[全実行結果](evidence/runs.json)を収録する。バイナリーのSHA-256は `9b9d5ba711f53cafb18d88600079938845c30ca6c5e93053c1e05414b0e10a93`。起動記録の Git revision が検証スクリプト追加後のコミットを指す場合、製品側の `editor-state.json` の revision とビルド記録を優先する。

## 受け入れ条件と証跡

| 条件 | 今回の判定 | 実行と照合対象 |
| --- | --- | --- |
| 6枚の代表例、全Element、構成変更、Overview往復、保存、UndoとRedo、再起動、両開始位置の発表 | 通過 | `accepted/sample`。保存KDLの期待値と全バイトの履歴復元を照合する |
| GUIだけで白紙から全種類を作成し、保存・再起動・発表する | 通過 | `accepted/blank`。起動時のデモをGUIで全削除して0枚にし、本文・画像・2列の3枚を作る。KDLを入力へ使わない |
| 0枚、空Slide、長い見出し、画面外Element、欠落画像 | 通過 | `overview/empty`・`overview/layout-*`・`overview/representative`、`accepted/inspector` の画面外編集 |
| 100枚のスクロール、ドラッグ、自動スクロール、格子、更新、画像再読み込み | 通過 | `workflows/overview-main`・`workflows/overview-drag`・`workflows/navigator`、`overview/slide-organization`。選択・原文・履歴・実画面を照合する |
| 3寸法、ペイン開閉と幅、Fitと100%、明暗、共通配置と縮小表示 | 通過 | `final/shell-light`・`final/shell-dark`・`final/shared-layout`、`overview/layout-*`・`overview/navigator-*`。倍率を旧ペイン幅の定数から推測せず、実際のキャンバス寸法を記録する |
| 標準日本語IMEの変換・再変換、表示・メニュー・保存・Undo・発表 | 不合格 | `final/ime-save-shortcut` で #77 を再現。Overview・保存メニュー・確定後のショートカット・再変換・発表は通過。各wrapperで入力ソースを復元 |
| 外部競合、無効入力、保存失敗と復旧 | 通過 | `accepted/document-status`・`accepted/document-present`、`overview/guards`。実ファイルと専用ディレクトリーの権限変更で確認する |
| キーボードとVoiceOver、操作名・選択・無効状態・フォーカス | 通過 | `accepted/keyboard-focus`、`accepted/voiceover-insertion`、`keyboard-voiceover/voiceover-overview`、`voiceover-final/voiceover-document`、`recheck/voiceover-inspector`・`voiceover-settings`・`voiceover-width`、`confirm/voiceover-offscreen`。実際の発話・操作・保存値・設定復元を照合 |
| 公開操作の自動テストと旧GUI導線の更新 | 通過 | `cargo test --locked --all-targets --features gui` は123件通過。旧ラベル、ドラッグ終端、キャンバス寸法、フォーカス取得を実機に合わせて更新 |

## 判定の扱い

コマンド送信の成功を操作結果の証明に使わない。画面・製品状態・KDLのいずれかの照合が失敗したケースは不合格とする。未実施と判定不能も合格へ含めない。

既存 [#31](https://github.com/daaa1k/psycho/issues/31)、[#46](https://github.com/daaa1k/psycho/issues/46)、[#73](https://github.com/daaa1k/psycho/issues/73)、[#77](https://github.com/daaa1k/psycho/issues/77) を解決済みとして扱わない。必須の描画確認は単一画面で実行した。VoiceOverのOverview100枚は1キーごとの選択を確認して進む手順。連打する `--burst` は今回未実施であり、#73の修正確認には使わない。

## 保存値と比較画像

| 手順 | 保存値・観測 | 比較画像 |
| --- | --- | --- |
| 代表例を全種類編集し構成変更 | [原本](evidence/accepted/sample/original.kdl)、[保存KDL](evidence/accepted/sample/presentation.kdl)、[観測](evidence/accepted/sample/sample-workflow-observations.json.gz)。各変更で保存、Undoによる全バイト復元、Redoを検査 | [編集](evidence/accepted/sample/sample-heading.png)、[Overview](evidence/accepted/sample/sample-overview.png)、[発表](evidence/accepted/sample/sample-presentation-current.png)、[復帰](evidence/accepted/sample/sample-return.png) |
| 白紙から全種類を作成 | [0枚](evidence/accepted/blank/zero-slides.kdl)、[保存KDL](evidence/accepted/blank/presentation.kdl)、[観測](evidence/accepted/blank/blank-authoring-observations.json.gz)。0枚のKDLはGUI観測のsourceHexから抽出した値で、入力に使っていない | [0枚](evidence/accepted/blank/zero-slides.png)、[保存後](evidence/accepted/blank/saved-all-six-kinds.png)、[画像](evidence/accepted/blank/presenting-authored-image.png)、[2列](evidence/accepted/blank/presenting-authored-columns.png) |
| 100枚の格子と自動スクロール | `overview/layout-*` と `workflows/overview-drag` の観測で順番と履歴を検査 | [狭い明色](evidence/overview/layout-light/1024x700-first.png)、[広い暗色の末尾](evidence/overview/layout-dark/1440x960-tail.png)、[下端](evidence/workflows/overview-drag/stationary-bottom-moved.png)、[上端](evidence/workflows/overview-drag/stationary-top.png) |
| 画像ファイルを書き換えて再読み込み | `overview/representative`。赤から青への画素変更と占有面積、元の赤い画素の消失を照合 | [変更前](evidence/overview/representative/all-elements-overview.png)、[再読み込み後](evidence/overview/representative/asset-updated-in-grid.png) |
| 編集と全画面の共通配置 | [寸法と倍率](evidence/final/shared-layout/layout-observations.json.gz)、[glyph比較](evidence/final/shared-layout/layout-glyph-comparison.csv.gz)、[raster比較](evidence/final/shared-layout/layout-raster-comparison.csv.gz) | [1024](evidence/final/shared-layout/layout-compact-1.png)、[1280](evidence/final/shared-layout/layout-small-1.png)、[1440](evidence/final/shared-layout/layout-large-1.png)、[全画面](evidence/final/shared-layout/layout-fullscreen-1.png) |
| 日本語変換中の保存 | [観測](evidence/final/ime-save-shortcut/observations.json.gz)のmarkedとsave。入力は日本語の変換、markedは[0,6]、composingはtrueのまま。ディスクはOriginalのまま | [保存前](evidence/final/ime-save-shortcut/marked-before-save.png)、[保存後](evidence/final/ime-save-shortcut/after-save.png) |
| 不正・削除された外部ファイルから復旧 | [観測](evidence/accepted/document-status/observations.json.gz)、[退避KDL](evidence/accepted/document-status/archive/presentation.kdl) | [不正](evidence/accepted/document-status/external-invalid.png)、[削除](evidence/accepted/document-status/external-deleted.png)、[退避後](evidence/accepted/document-status/retreat-preserves-draft-clears-history.png) |

[KDL差分](kdl-differences.patch)は代表例の原本から最終保存、および白紙0枚の観測から最終保存を比較する。Slideの移動と元への復帰は最終差分に残らないため、観測記録と実行ログを併読する。

共通配置はglyph ID、正規化座標、raster strokeを照合した。既存スクリプトには常に成立する量子化チェックがあったため除去し、保存済み画像とトレースで再照合して通過した。量子化そのものの合格とは主張しない。代表例の画像パス変更は同じ絵の複製を使う。異なる絵への更新は `overview/representative` の再読み込み・画像照合を根拠とする。白紙の画像スライドは相対パスとCaptionの自動照合に加えて保存画像を目視する。

## 再実行

macOSのGUIセッション、XcodeのSwift、Orcaのcomputer機能とアクセシビリティ権限が必要。描画照合のPython環境にはPillowとnumpyを入れる。出力は毎回新しい `target/` 以下のディレクトリーを使う。GUIテストは並列に実行しない。

```sh
cargo test --locked --all-targets --features gui
PSYCHO_BUILD_REVISION="$(git rev-parse HEAD)" cargo build --locked --features gui
PSYCHO_EDITOR_STATE="$PWD/target/issue-56-rerun/sample/editor-state.json" python3 validation/mvp-acceptance/sample_workflow_acceptance.py --output target/issue-56-rerun/sample
PSYCHO_EDITOR_STATE="$PWD/target/issue-56-rerun/blank/editor-state.json" python3 validation/mvp-acceptance/blank_authoring_acceptance.py --output target/issue-56-rerun/blank
```

各ケースの正確な引数と終了コードは `evidence/runs.json` に記録する。IMEのコマンドは `with_japanese_ime.swift` を通し、入力ソースの復元まで確認する。VoiceOverのケースは開始時にVoiceOverを停止し、終了後のrestoration.jsonで元の設定への復元を確認する。

[入力境界の設計](driver-design.md)には比較案と選択理由を記録する。配送receiptは操作成功の証明に使わない。入力欄の完全一致、所有するPIDと実行ファイル、同じAX要素のフォーカス、製品状態、保存ファイルを段階ごとに照合する。

## 実行記録の読み方

`overview`、`workflows`、`final`、`accepted` は実行順の名前であり、総合合格の意味ではない。初回の旧ラベル・座標・フォーカス・非同期読み戻しによる停止も残す。再実行は `recheck`、`confirm`、`keyboard-voiceover`、`voiceover-final` に分け、結果を書き換えていない。

VoiceOverのInspector一覧は、カーソル文字列が繰り返されたら一覧を抜ける旧手順をやめ、製品が備える上下キーによる選択とスクロールを使う。設定入力はTabで欄へ移り、実際の発話と欄のフォーカスを照合する。保存はCmd+Sを送り、期待する文字列が実ファイルへ書かれるまで確認する。診断はVoiceOverでボタンを操作し、取消後の入力欄からはinteractionを終了して移る。101枚目の追加、読み上げ、保存、Undoも確認した。

[manifest](evidence/manifest.json)は元の相対パス・SHA-256とアーカイブのSHA-256を対応付ける。テキストはmtime=0のgzip、KDLと選択画像は元のバイトで収録する。選択画像に対応するAX JSON、起動記録、入力receipt、観測、設定復元も収録する。ネイティブ入力の試作probeログだけはPASS行を抽出し、その変換と元ログのハッシュを記録する。保存パネルの巨大なファイル一覧や実行バイナリーは収録しない。保存原文のKDL2ファイルには行末空白があり、git diff --checkはこの2箇所を報告する。原文との全バイト一致を保つため、そのまま収録した。

[全実行結果](evidence/runs.json)の終了コードを各ケースの結果として読む。未実施をファイルの存在から推定しない。[SHA256SUMS](evidence/SHA256SUMS)でアーカイブ全体を検査できる。

```sh
cd validation/editor-acceptance/evidence
shasum -a 256 -c SHA256SUMS
```

[設計比較](review/driver-design.md)、[入力・保存の独立レビュー](review/driver.md)、[最終証跡レビュー](review/verdict.md)、[判断の記録](decisions.tsv)を併せて公開する。Prove It Worksに従い、実画面と保存値の両方を根拠にした。Build the Leverに従い、証跡の収集とハッシュ対応を `collect.py` で再実行できるようにした。
