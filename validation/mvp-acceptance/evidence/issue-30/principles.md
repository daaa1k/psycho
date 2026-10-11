# 判断に使ったpstackの原則

このセッションでleaf SKILL.mdを読んだ原則と、判断を変えた箇所を記録する。

| 原則 | この作業で選んだ具体的な方法 |
| --- | --- |
| Prove It Works | 過去の合格だけで終えず、公開操作・実ファイル・現在のnative GUI・実録画を再検証する |
| Build the Lever | 検証を一度の手操作で終えず、30ケースの実行器とハッシュ照合・再実行手順を残す |
| Laziness Protocol | 公開APIと切り出し範囲を維持し、引用外の物理行頭だけを修正する。再変換は既存の履歴開始値の条件1行を直す |
| Model the Domain | KDLの行境界と引用範囲を共通の処理で扱う。変換セッションは見えるmarked範囲ではなく既存のcomposition_beforeで識別する |
| Fix Root Causes | 字下げの原文不一致、OSメニュー規則、自動有効化、再変換の実コールバックを再現して原因を修正する |
| Test Behavior, Not Implementation | モデル一致だけでは見逃すコメント・引用・slashdashの欠落を、独立した期待原文全体・保存・再読込・Undo/Redoで検査する |
| Exhaust the Design Space | 字下げ修正の複数案を比較し、引用保護・タブ推論・inlineの処理を小さい差分にまとめる |
| Attack the Premise | 小さいソース行をOCRで正確に読めるという前提を捨て、行位置の特定と実描画の双方向比較を分ける |
| Sequence Work into Verifiable Units | 原文とnativeの失敗する検証を先にコミットし、修正を次のコミットに分ける |

適用箇所の証拠は[決定ログ](decisions.tsv)、[再現経過](history/README.md)、[独立レビュー](reviews.md)、[受け入れ記録](../../issue-30.md)にある。
