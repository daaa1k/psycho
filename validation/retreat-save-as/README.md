# Issue #28 退避保存

`PresentationDocument` の公開操作を実ファイルで検証し、ネイティブ保存パネルと入力履歴をGUI実機検証で補う。原本とAssetのバイト列、新しい保存先、保存失敗前後の入力履歴を比較する。

## GUI検証の再実行

macOS、Xcode Command Line Tools、Orcaのアクセシビリティ・画面収録権限が必要。検証スクリプトは専用のアプリプロセスと `target/` 内のファイルだけを操作する。`--output` には未使用のディレクトリーを指定する。

```sh
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang
export CC=/usr/bin/clang
export CXX=/usr/bin/clang++
cargo build --features gui --bin psycho
python3 validation/retreat-save-as/gui_acceptance.py --output target/retreat-save-as-gui
python3 validation/retreat-save-as/gui_acceptance.py --case focused-image --output target/retreat-focused-image
```

`observations.json` には画面のOCR、保存パネルのアクセシビリティツリー、入力内容とUndo / Redo件数を記録する。各確認点のスクリーンショットも同じ出力先へ保存する。

## 実行結果

macOS 26.5.2（25F84）、Apple Silicon、GPUIリビジョン `14dd03e89676e7fe74bc205001bfb32c8cfc3952` で検証した。ウィンドウは1280×892、スクリーンショット倍率は1。

`cargo test --features gui` の65テストが通過した。[実行ログ](evidence/cargo-test.txt)を参照。新しい `tests/retreat_save_as.rs` は次の動作を公開操作で検証する。

- 競合、構文エラー、削除からの退避。失敗時の編集対象・内容・履歴保持と、再試行成功後の保存済み基準・編集再開。
- 原本、親フォルダーの別名、symlink、hardlinkを保存先に指定した場合の拒否。原本削除後の別名とリンク循環も含む。
- 列内を含む複数画像、欠損画像、日本語・空白を含むパス、symlinkの後に `..` がある参照。同じフォルダーへの退避ではraw stringの表記も保持する。
- 原本のフォルダーが削除された後の退避。

GUIは[11確認点](evidence/gui-result.txt)が通過した。[観測記録](evidence/observations.json)に入力履歴と画面表示を記録した。

| 操作 | 確認内容 |
| --- | --- |
| 保存前の予告 | [履歴消去と画像パス差分の予告](evidence/01-before-save-notice.png)を保存先選択前に表示した |
| 予告・保存パネル・上書き確認の取消 | 原本と入力を保持し、入力欄のUndo / Redo件数も各1件のまま保った |
| 書込み失敗 | 保存先フォルダーを一時的に書込み不可にして[失敗](evidence/03-write-failure.png)を再現し、入力・履歴を保持した |
| 原本への退避 | ネイティブ上書き確認で承認しても[原本への退避を拒否](evidence/06-original-rejected.png)し、外部変更を保持した |
| 別フォルダーの既存先へ再試行 | 承認後に[退避に成功](evidence/08-retreat-success.png)し、履歴を消去した。GUIで編集していない3つの画像参照だけを追加で書き換え、Asset自体は変更しなかった |
| 退避後の編集・表示 | Captionと[列内の画像](evidence/10-nested-images.png)を表示し、新しい保存先で編集・通常保存を再開した |

画像パス入力欄にフォーカスがある状態からの退避も[通過](evidence/focused-image-result.txt)した。[入力欄を新しい相対パスへ更新](evidence/focused-image-after-save.png)し、その後の通常保存でも旧パスへ戻らないことを確認した。
