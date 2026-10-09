# Issue #27 外部変更からの復旧

`tests/external_changes.rs` は一時ディレクトリー内の実ファイルを変更・削除し、GUIと共有する `PresentationDocument` の公開操作を検証する。

| ケース | 確認内容 |
| --- | --- |
| 正常な外部変更 | 未保存編集がなければ再読み込みし、履歴を消去する。新しい内容を基準に保存できる |
| 未保存編集との競合 | 編集・保存・Undo / Redoを止め、内容と履歴を保持する。明示再読み込み成功後だけ制限を解除する |
| 構文・Schemaエラー | 外部ファイルの診断と該当行を取得し、保持したGUI内容を置き換えない。失敗した再試行でも履歴を保持する |
| 外部削除 | 削除を識別し、保存でファイルを復活させない。復元しても明示再読み込みまで制限を維持する |
| 入力欄だけに変更がある場合 | 自動再読み込みを止め、再読み込み失敗時にも文書とRedo履歴を保持する |
| 保存直前の外部無効化 | 定期確認を待たずに上書きを止め、診断と履歴停止へ移る |

実行コマンドは次のとおり。macOSではシステムのClangを指定し、Nix側のリンカーとの混在を避ける。

```sh
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang
export CC=/usr/bin/clang
export CXX=/usr/bin/clang++
cargo test --test external_changes
cargo check --features gui
cargo test --features gui
```

## GUI実機検証

macOS 26.5.2（25F84）、Apple Silicon、GPUIリビジョン `14dd03e89676e7fe74bc205001bfb32c8cfc3952` で実施した。編集ウィンドウは1280×892、Computer Useのスクリーンショット倍率は1。全画面発表への移行とEscapeでの復帰も確認した。

[gui_acceptance.py](gui_acceptance.py) は検証専用のアプリプロセスと一時KDLを作り、`orca computer` で実際の入力・クリック・キー操作・ネイティブ保存パネルを操作する。画面のOCRとスクリーンショット、ファイルのバイト列、GPUIの入力状態ログを照合する。入力状態ログでは内容、Undo / Redo件数、フォーカスも確認する。検証用プロセスだけを終了し、既存のユーザーファイルは変更しない。

全21確認点が通過した。[実行結果](evidence/gui-result.txt)と[観測記録](evidence/observations.json)を保存している。

| 操作 | 観測結果 | 証跡 |
| --- | --- | --- |
| 正常な外部変更 | 未保存編集がなければ自動で再読み込みした | [自動更新](evidence/01-clean-auto-reload.png) |
| 文書の編集と未確定入力を残した競合 | 両方を保持し、保存・Undo / Redo・両方の発表開始操作を停止した。原本は外部版のまま | [停止中の画面](evidence/03-conflict-frozen.png) |
| 再読み込みの確認とキャンセル | 破棄を確認する操作が表示され、キャンセル後も入力を保持した | [確認画面](evidence/04-discard-confirmation.png) |
| 構文・Schemaエラーでの再試行 | 再読み込みは失敗し、最後の正常表示・未確定入力・履歴を保持した。診断を選ぶと外部ファイルの読み取り専用ソースを表示した | [構文](evidence/06-syntax-readonly-source.png)、[Schema](evidence/08-schema-readonly-source.png) |
| 外部削除と再試行 | 削除と表示の不一致を表示し、保存操作でも原本を復活させなかった | [削除](evidence/09-deleted-failed-retry.png) |
| 正常ファイルへ修復して再読み込み | 履歴を消去し、編集・保存・全画面発表を再開できた | [発表](evidence/11-presentation-restored.png)、[編集保存](evidence/12-edit-save-restored.png) |
| 未保存編集のない削除・無効化 | 定期確認で検出し、修復後の再読み込みで復帰した | [削除](evidence/13-clean-deletion.png)、[無効化](evidence/15-clean-invalid.png)、[復帰](evidence/17-clean-invalid-recovered.png) |
| 競合中の未確定入力を含む別名保存 | 保存先を書込み不可にすると失敗し、元ファイル・入力・履歴を保持した。権限を戻して再試行すると入力を含めて保存し、履歴を消去した | [失敗](evidence/19-retreat-failed.png)、[成功](evidence/20-retreat-recovered.png) |
| 別名保存後の編集 | 新しい保存先で編集・保存でき、元ファイルは外部版を維持した | [編集保存](evidence/21-retreat-edit-save.png) |

実機検証で、外部診断の長いソース行が画面右端で切れる問題を確認した。診断パネルの幅を制約し、折り返しと縦スクロールを追加して修正した。行末の `visible_tail` を検出する検証は修正前に失敗し、修正後に成功した。[修正前](evidence/diagnostic-width-before.png)と[修正後](evidence/diagnostic-width-after.png)を保存している。

修正後の `cargo test --features gui` も全60テストが通過した。[ログ](evidence/cargo-test.txt)を参照。

## GUI検証の再実行

Orcaのアクセシビリティ・画面収録権限とXcode Command Line Toolsが必要。上記の環境変数を設定してから実行する。`--output`には未使用のディレクトリーを指定する。

```sh
cargo build --features gui --bin psycho
python3 validation/external-changes/gui_acceptance.py --output target/external-changes-gui
# 診断ソースの表示幅だけを検証する場合
python3 validation/external-changes/gui_acceptance.py --case diagnostics --output target/external-diagnostics-width
```
