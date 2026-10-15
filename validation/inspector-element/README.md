# Inspector の操作を再検証する

macOS 上で `target/debug/psycho` をビルドし、Orca のアクセシビリティ権限を有効にする。Python 3、Xcode command line tools、`orca`、`psycho-verify` が必要。各スクリプトは専用の GUI プロセスと KDL を作成する。`--output` には未使用の `target/psycho-verify/` 配下を指定する。同時に実行しない。

リポジトリのルートで、操作前に scratch プロセスを起動して doctor を実行する。

```sh
psycho-verify launch --run-id inspector-preflight --kdl examples/build-time.kdl
psycho-verify doctor --run-id inspector-preflight
psycho-verify cleanup --run-id inspector-preflight
```

Inspector の基本操作、全文編集、保存、再読込、外部変更の拒否を検証する。

```sh
python3 validation/mvp-acceptance/inspector_element_acceptance.py --output target/psycho-verify/inspector-rerun/main
```

途中の PASS だけで成功を判定しない。終了コードが 0 であることを確認する。途中で失敗した場合も `observations.json` に成功した項目が残る。

キーボード操作と小さいウインドウを、それぞれ新しいディレクトリで検証する。

```sh
python3 validation/inspector-element/controls.py --output target/psycho-verify/inspector-rerun/controls
python3 validation/inspector-element/controls.py --outline-only --output target/psycho-verify/inspector-rerun/outline
python3 validation/inspector-element/controls.py --footer-only --output target/psycho-verify/inspector-rerun/footer
python3 validation/inspector-element/long_code.py --output target/psycho-verify/inspector-rerun/long-code
python3 validation/inspector-element/row_focus.py --output target/psycho-verify/inspector-rerun/row-focus
python3 validation/inspector-element/row_focus.py --exposure-only --output target/psycho-verify/inspector-rerun/row-exposure
python3 validation/inspector-element/navigator_helper.py --output target/psycho-verify/inspector-rerun/navigator
python3 validation/inspector-element/conflict.py --output target/psycho-verify/inspector-rerun/conflict
```

保存と周辺機能への影響を検証する。

```sh
python3 validation/mvp-acceptance/column_width_drag_acceptance.py --output target/psycho-verify/inspector-rerun/divider
python3 validation/mvp-acceptance/layout_repair_acceptance.py --output target/psycho-verify/inspector-rerun/layout
python3 validation/mvp-acceptance/insertion_reload_present_acceptance.py --output target/psycho-verify/inspector-rerun/insertion
```

日本語 IME の変換中操作を検証する。wrapper は元の入力ソースと有効状態を復元する。

```sh
xcrun swift validation/mvp-acceptance/with_japanese_ime.swift /usr/bin/python3 validation/mvp-acceptance/inspector_element_acceptance.py --ime --output target/psycho-verify/inspector-rerun/ime-main
xcrun swift validation/mvp-acceptance/with_japanese_ime.swift /usr/bin/python3 validation/inspector-element/marked_keys.py --output target/psycho-verify/inspector-rerun/ime-keys
```

VoiceOver をオフにしてから、ネイティブの読み上げと操作を検証する。スクリプトは専用アプリの生存に依存せず VoiceOver を停止し、初回案内画面も閉じる。起動時設定を `restoration.json` に記録し、終了時に復元する。AX ツリーだけでは読み上げの成功を判定できない。

```sh
python3 validation/inspector-element/voiceover.py --output target/psycho-verify/inspector-rerun/voiceover
python3 validation/inspector-element/voiceover.py --settings-only --output target/psycho-verify/inspector-rerun/voiceover-settings
python3 validation/inspector-element/voiceover.py --width-only --output target/psycho-verify/inspector-rerun/voiceover-width
python3 validation/inspector-element/voiceover.py --offscreen-only --output target/psycho-verify/inspector-rerun/voiceover-offscreen
```

各実行の標準出力と標準エラーをログに保存する。`observations.json`、保存した KDL、`saved.diff` がある場合はその差分、関連する PNG、入力状態、glyph trace も同じ実行単位で保管する。実行前に `git rev-parse HEAD` と `git diff` を保存し、未コミット変更を含めて実行したソースを特定する。

現在の記録と公開用の証拠は [manifest.json](evidence/manifest.json) にある。各証拠のパスは evidence ディレクトリを基準とする。実行時の commit を保存していない履歴は revision=null とし、最終ソースでの成功と区別する。ログ、観測値、保存した KDL と代表画像を同梱する。大きな AX 観測 JSON は `.json.gz` に可逆圧縮し、`gzip -dc <path>` で読める。大きな glyph CSV の全体は同梱せず、観測 JSON に比較した座標を残す。

IME の座標検証は候補本体と補助ウィンドウを両方確認する。同じ変換中の Tab、Shift-Tab、Canvas 往復は 3px 以内、別の変換との比較は 8px 以内とする。別の変換で観測した 4px の差の原因は未特定。標準日本語 IME を macOS 26.5.2 で検証した結果であり、別の IME と複数ディスプレイの候補配置は未検証。

最終の主受入、IME の候補座標、画面外の VoiceOver 選択・編集・保存、幅の読み上げと設定の自動復元は通過した。コード言語と全文の VoiceOver 編集は実機で保存まで通過し、その実行で失敗した旧終了処理と手動復旧も記録した。修正した終了処理は別の実機ランで自動復元まで通過した。失敗した過去のランを全体成功として扱わない。
