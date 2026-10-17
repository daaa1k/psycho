# Slide Overview の実機検証記録

macOS 26.5.2 の単一ディスプレイで、所有する scratch プロセスだけを直列に操作した。17工程の採用結果を [manifest.json](manifest.json) に記録する。各工程の実際のリビジョン、バイナリの SHA-256、コマンド、終了コードを残している。単一リビジョンで全工程が一括成功した記録ではない。

| リビジョン | 採用した工程 |
| --- | --- |
| `fe10c38` | empty、drag、resize、layout-light、layout-dark |
| `35d976d` | navigator-light、navigator-dark、slide-organization |
| `c8c3f83` | voiceover |
| `fbb2a63` | invalid、representative、ime |
| `8f9d900` | main |
| `811dd32` | guards、diagnostics |
| `f1ef547` | navigator、slide-guards |

`main` は Inspector の希望幅と非表示設定を保持し、無変更操作、並べ替え、Undo/Redo、直接編集、保存、発表、Escape による同一 Slide への復帰まで通過した。保存した原文は [main/saved.kdl](main/saved.kdl) にある。`diagnostics` は Overview の画像診断から Edit の該当 Slide と画像へ移動し、原文、履歴、保存ファイルを変えないことを確認した。

layout の各工程は 100 枚を 1024×700、1280×860、1440×960 で操作し、末尾への到達を確認した。これは処理速度の測定ではない。`representative` は全 Element、空 Slide、画像の明示的な再読み込み、外部変更、構文不正時の最後の正常表示を確認した。

VoiceOver は実際のカーソルと発話を確認した。項目の選択、両方向の並べ替え、Undo/Redo、97回の Right による3枚目から100枚目への移動、Return での Edit 復帰を確認した。各 Right の後に選択を確認し、キーを再送していない。[voiceover/keyboard-transitions.json.gz](voiceover/keyboard-transitions.json.gz) に97件の一致がある。VoiceOver と IME の設定復元も記録している。

[unit-tests.json](unit-tests.json) は GUI 有効123件、既定113件とフォーマットの成功記録を示す。[product-source.json](product-source.json) は、そのテスト時点と最終実機ビルドで Cargo と Rust 全ファイルの blob が同じことを示す。以前の実機リビジョンとの差は [fe10 以後](changes-after-fe10.patch.gz) と [35d9 以後](changes-after-35d9.patch.gz) に保存した。追加された入力境界とネイティブフォーカスの変更は、後続の guards、invalid、main、VoiceOver、Navigator で確認した。

各工程の `observations.json.gz` に操作前後の文書、履歴、選択、AX tree がある。Navigator の競合・初回構文不正の補助ケースは同ディレクトリの下位記録にある。`.gz` は `gzip -dc <file>` で読める。再実行は [検証手順](../README.md) を参照する。

## 失敗した試行と制限

[trials.json](trials.json) に途中の失敗と後続の成功を残した。失敗ログは [failures/trials](failures/trials) にある。修正前の製品の問題と、古い項目名、誤った画像更新の前提、入力や観測の失敗を合格に数えていない。

[failures/native-input-driver](failures/native-input-driver) は同一プロセスでの入力経路の比較を含む。Orca のクリックや Select All が期待するフォーカスと選択を作らず、ネイティブ入力では完全置換が成功した。入力ヘルパー本体とその独立した証跡は [PR #75](https://github.com/daaa1k/psycho/pull/75) に分離した。途中の provider `app_not_found` は PID が生存していたが、その工程を成功とは扱わず、新しいプロセスで再実行した。

VoiceOver 有効時の高速な連続入力では、遅延した AX Click が古い項目を再選択する問題が残る。[Issue #73](https://github.com/daaa1k/psycho/issues/73) で追跡している。通常のキー長押しでの再現有無は未確認。順次操作による到達性の成功は、この選択の問題の解決や処理速度を証明しない。`failures/voiceover-selection-burst` と `failures/focus-diagnosis` は探索用の追加計測を含み、採用した成功記録とは分けている。
