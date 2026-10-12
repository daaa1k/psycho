# Slide の追加・削除・並べ替えの実機検証

[Issue #51](https://github.com/daaa1k/psycho/issues/51) の検証です。製品コードのリビジョンは `7675694ea290aa27c429ef2213ebad79d443b787` です。

## 確認結果

| 対象 | 結果 | 証跡 |
| --- | --- | --- |
| 公開文書 API と既存 GUI テスト 118 件 | PASS | `evidence/cargo-test.log` |
| 100 枚の末尾から先頭へドラッグ、静止ポインターで端スクロール、挿入線、移動対象の表示 | PASS | `evidence/native/observations.json` と PNG |
| Escape、元位置、一覧外、一覧見出しへのドロップで原文・dirty・Undo・Redo・draft・選択が不変 | PASS | 同上 |
| 右クリックによる対象選択、4 操作、先頭の移動不可、Escape と実行後のフォーカス復帰 | PASS | `acceptance.py` と同上 |
| Slide メニューと Navigator 専用キーの追加・移動・削除、1 回の Undo と Redo | PASS | 同上 |
| 保存と再読み込みで原文と順序が一致 | PASS | 同上、`original.kdl`、`moved.kdl`、`saved.kdl` |
| 0 枚から追加、最後の Slide の削除と Undo | PASS | `evidence/guards/observations.json` |
| 無効な列幅入力で追加とドラッグ移動を拒否、取消と無操作で入力を保持 | PASS | 同上 |
| 外部競合で原文と未確定入力を保持し操作を無効化、入力中の Delete と矢印を優先 | PASS | 同上 |
| 削除した Slide の内容を Undo で復元し、参照 Asset を削除しない | PASS | `tests/slide_organization.rs` と `evidence/cargo-test.log` |

`acceptance.py` はコメント、明示 ID、raw 文字列を含む 100 枚の KDL を生成します。実ウィンドウにポインターイベントを送り、AX の操作、macOS メニュー、キーボードを使います。画面を取得し、公開操作後の原文と履歴を照合します。`guards.py` は空文書と無効入力・外部競合を別の検証用ウィンドウで操作します。両スクリプトは指定リビジョンを各観測時に確認します。

macOS 26.5.2、Apple M3、内蔵 Retina 画面で実行しました。画面情報は `evidence/native/result.json` に記録しています。この検証は性能測定ではありません。VoiceOver の実発話、日本語 IME、複数画面は今回の検証に含めていません。既存の #31、#46 と Navigator 検証の残件を解消したとは扱いません。

独立レビューの読み上げ名とフォーカスの指摘を修正しました。Escape はドラッグ中なら取消、通常なら既存のメニュー取消へ伝播します。一覧外の取消とドロップの判定範囲を同じ一覧に揃えました。見出しへのドロップで状態が残るという静的な懸念は旧版の実機で再現せず、再現済み不具合とは扱っていません。

## 再実行

Orca の Accessibility と画面収録接続、Xcode、Python 3 が必要です。GUI ドライバーを同時に動かさず、新しい `target/` ディレクトリを指定します。

```sh
revision=$(git rev-parse HEAD)
env CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang PSYCHO_BUILD_REVISION="$revision" cargo test --features gui
env CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang PSYCHO_BUILD_REVISION="$revision" cargo build --features gui
python3 validation/slide-organization/acceptance.py --output target/slide-organization-new --revision "$revision"
python3 validation/slide-organization/guards.py --output target/slide-organization-guards-new --revision "$revision"
```

Nix のリンカーが先に選ばれる環境では `-liconv` のリンクが失敗するため、Rust のリンカーを Xcode Clang に指定します。
