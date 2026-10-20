## Findings

### 1. [warning] 文書操作の状態遷移が `PsychoApp` に集中し、オプショナル引数の組合せで意味を表している
**Location**: `src/main.rs:1110` (`start_document_flow`)、`src/main.rs:1289`–`1305`（成功後の Open/Close/Reload 継続）
**Finding**: 保存・別名保存・開く・閉じる・再読み込みの非同期フローを一関数で扱う設計だが、`action: Option<DocumentAction>` と `save: Option<SaveIntent>` の組合せ、ループ後の `save.is_some()` による継続可否、フォーカス復帰条件の事後判定で状態を表している。フローの各状態が型で表されず、分岐を変更すると「保存を試みたが中断」「保存後に文書操作へ進む」「単独保存」の違いを壊しやすい。診断・保存失敗・外部状態の表示と遷移も同じ巨大な `PsychoApp` に追加されている（基準コミットから `src/main.rs` は 6,656 行から 6,948 行へ増加）。
**Evidence**: `start_document_flow` は確認、Save As 警告、保存先選択、再試行、文書操作の継続、フォーカス復帰を一つの非同期クロージャ内で直列化している。Open/Close/Reload 実行条件はループ後に `save.is_none()` で間接的に決まり、保存結果やキャンセル経路の変更時に追跡しづらい。今回の変更では `main.rs` に 765 行追加されている。
**Suggestion**: 次の変更で保存フローと文書操作フローを専用の小さな型/関数へ切り出し、確認済み・保存待ち・操作継続待ちを明示する。少なくとも `Option` の直積を `DocumentFlow` の列挙状態へ置き換え、継続操作とフォーカス復帰を状態遷移の結果として返す。

## Audit flags

- `.audit/issue-55.tsv` の最初の行が説明するとおり、記録日時はイベント発生時刻ではなく監査時点として扱った。各行の根拠パスは作業ツリーに存在し、レビュー対象の仕様・設計判断と矛盾する主張は見つからなかった。
- `target/issue-55-design/synthesis.md` と Issue #55/#48 の受け入れ条件を照合した。診断時の無効 draft 保持、外部無効ソース行への読み取り専用移動、VoiceOver 経由の操作を示す acceptance スクリプトと結果ログが確認できた。
- セッション digest で未完了と指定された親独立の無効 draft・非current画像・native IME・履歴確認は、完了扱いにしていない。現存するスクリプトや他の GUI ログだけでは、その個別確認が成功した証拠にならない。
- 指定どおり固定範囲 `246ad13...91822ca` を読んだ。GUI/build/source変更は行っていない。
