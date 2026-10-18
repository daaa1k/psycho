# Issue 55 の検証

保存状態、確認ダイアログ、外部変更からの復旧、診断と発表を、所有する scratch プロセスで操作した。全量の PNG・AX JSON・KDL・ログは `target/issue-55-*` に残している。このディレクトリは代表的な証跡を含む。

製品コードの最終リビジョンは `59eee9b`。macOS 26.5.2、build 25F84、接続画面は Color LCD 1 台、native 1920×1200。起動 metadata がある実行では、Git revision と実行バイナリーの SHA-256 を保存した。先行する status・open・source の実行は metadata 導入前の作業ツリーを使った。その内容と後続修正は `16382ab` から始まるコミット列で確認できる。

実行結果は以下のとおり。

- `status/result.log`。未確定入力の未保存表示、閉じる三択、Escape 取消後の入力とフォーカス、実権限による保存失敗、別の表示操作後も残る失敗表示、再保存、競合、再読み込みの取消と成功、外部無効化・削除、別名保存による退避が通過した。退避先の KDL、元ファイルの非変更、退避後の Undo 無効も照合した。
- `open/result.log`。保存先なしの初期状態、未確定入力のある Open、Save As 成功後の元の Open 継続を確認した。退避先には入力が入り、開いた文書の原本は変わらなかった。
- `source/result.log`。外部構文診断へ Tab・Return で進み、読み取り専用ソース行を表示した。Escape 後も正常プレビューと入力、外部原本を保持した。
- `draft-guard/result.log`。空のタイトル入力から別 Slide の配置診断を選んでも、Slide・編集対象・入力・原本が変わらない。診断一覧を保持し、Escape 後は元入力へフォーカスが戻る。
- `voiceover/result.log`。実際の VoiceOver cursor と最後の発話を採取した。未保存確認とキャンセル、画像診断への移動、再読み込みへの到達、取消と Return による破棄、発表開始と Escape 帰還を操作した。`restoration.json` は VoiceOver と scripting・welcome 設定の復元結果を示す。後続の静的状態ラベル追加後には、全 VoiceOver シナリオを再実行していない。
- `presentation/result.log`。現在から・最初からの全画面開始、未確定入力の確定、全画面中の移動、Escape 後の選択復元、自動保存をしないことを照合した。
- `layout/result.log`。明暗それぞれで、編集領域 1024×700、1280×860、1440×960 のバナーと診断一覧を操作した。対応する native window はタイトルバー込みで 1024×732、1280×892、1440×992。サイズ変更前にウィンドウを移動し、CGWindow の実寸を直接照合した。provider が返す bounds と screenshot scale は別に保存した。

`issue-55-implementation-test-final.log` は core 113 件の成功を示す。最終 GUI build、`psycho-verify doctor` と map-lint、変更した Python の構文確認も成功した。Apple Clang を `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER`、`CC`、`CXX` に指定した。

日本語 IME の未確定変換を含む最終検証と、独立した無効入力・履歴・非現在画像の再検証は統合担当が実行する。Save As の書き込み失敗専用 script は新しい retry modal に合わせて更新したが、この実装実行では再実行していない。新しい保存失敗の実証は通常保存の実権限エラーによるもの。既存 Issue #31・#46 を解決済みとは扱わない。

再実行は `validation/mvp-acceptance/document_*_acceptance.py`、`external_diagnostic_acceptance.py`、`diagnostic_draft_retention_acceptance.py` に、新規の `target/` 配下の `--output` を指定する。`results.json` は各実行名と代表成果物の SHA-256 を持つ。

## 保存後の履歴と日本語 IME の追加検証

`4e800c0` は、保存成功後に入力欄へフォーカスを戻していた回帰を修正する。入力欄の履歴を文書へ移した後、入力欄の空の Redo が Cmd+Shift+Z を受け取っていた。成功時は文書へフォーカスを移す。失敗と取消では元のフォーカスを戻す。

`input-repair/redo-red` は修正前の失敗を保持する。既存 `input_redo_transfer_acceptance.py` の `Save/edit completion must preserve Redo for undone input units` が失敗した。`input-repair/redo-green` は同じテストの成功を保持する。Redo と Undo の往復、および元の raw quote を含む全バイト列の復元を確認した。

`ime_save_acceptance.py` は macOS 日本語入力を実際に使う。`--entry menu` では、未確定変換から「閉じる」を選び、Escape で取り消した後の入力とフォーカスを確認する。その後「保存」を選び、変換解除と保存バイト列の一致を確認する。`--entry confirmed-shortcut` では Enter で変換を確定してから Cmd+S で保存する。いずれも成功した。元の入力ソースと有効化状態も復元した。

変換中の Cmd+S は未解決であり、[Issue #77](https://github.com/daaa1k/psycho/issues/77) で追跡する。`--entry shortcut` はこの不具合を失敗として検出する。旧版 `246ad139a81263200212b956748138b33636ecd2` の同等手順でも変換範囲が残り、ディスク内容は更新されなかった。#55 の新規回帰ではない。固定している GPUI の `gpui_macos/src/window.rs` の `handle_key_event` は、変換中のイベントを `inputContext.handleEvent` へ先に渡し、処理済みならアプリへ渡さない。この分岐は Cmd キーを除外していない。メニューからの保存は成功している。依存ライブラリと native hook は変更していない。

次のコマンドの出力先は毎回新しいパスにする。

```sh
xcrun swift validation/mvp-acceptance/with_japanese_ime.swift /usr/bin/python3 validation/mvp-acceptance/ime_save_acceptance.py --entry menu --output target/ime-save-new-run
```

`input-repair` に入力状態、KDL、AX 状態、代表画像とログを保持した。旧版の `launch-1.json` の revision は helper 側の checkout を示すため、実行バイナリーの revision と SHA-256 は `ime-baseline/build-provenance.json` で訂正している。修正版を commit 前にビルドした実行についても `input-repair/build-provenance.json` に記録した。`all-feature-tests.log` は `cargo test --locked --all-features` の成功を示す。

`input-repair/document-status` は実権限による保存失敗と取消を含む文書フローの再検証である。取消後に入力文字列とフォーカスが残ることを追加で照合した。
