# Issue 53 設計比較

sol案を土台とし、astra案の「拒否を伝播する言語候補適用」を組み込む。全案とも実装前の設計であり、点数は実機での動作保証ではない。

| 候補 | 受け入れ動作の網羅 | 選択・draft・IME・履歴の単一所有 | 長い構造と対象設定への到達 | キーボード・VoiceOver | APIの深さ・層の少なさ |
| --- | --- | --- | --- | --- | --- |
| astra | 4 | 5 | 3 | 4 | 5 |
| sol | 4 | 5 | 4 | 4 | 4 |
| luna | 3 | 4 | 3 | 3 | 2 |

astraは確定失敗と外部競合を最も具体的に扱う。Inspector全体を一つのスクロールにすると、構造一覧の末尾から選択するたび先頭に戻り、連続選択に戻り操作が必要になる。対象設定の到達を優先する理由は明確だが、solの専用構造スクロールの方がこの操作を保ちやすい。

solは専用構造スクロールと既存編集状態の組み合わせがよい。コード例では `select_current_field` の成否を確認せず `set_value` へ進み、本文にある「失敗時は変更しない」と食い違う。この点は採用前に修正する。専用スクロールだけでは、外側Inspectorが下端にいる際の対象設定の見え方までは保証しない。設定領域と構造領域の配置を明示し、末尾の構造行を選んでも対象見出し・主要入力が見えることを受け入れ条件にする。

lunaは必要な方向を押さえるが、`OutlineTarget` と `ColumnSide` は既存 `CanvasSource` と列指定を重複させる。`select_outline`、`select_background` も既存入口を転送するだけなら不要。`InspectorContext` を導出に限定する注意はよいが、この規模では直接matchで足りる。候補適用の失敗、スクロールAPI、キーボード操作の記述が他案より弱い。

## 統合する具体事項

1. solの全行表示と専用 `ScrollHandle` を採用し、`OutlinePage` とページ操作を同じ変更で削除する。行の選択正本は `EditTarget` のままとする。`OutlineRow` は描画に必要なsource・label・depthだけを持つ導出値にする。
2. astraの非公開 `apply_code_language` を採用する。外部競合確認、旧draft確定、言語欄選択、候補設定、確定をこの入口へ集約する。既存選択APIが返す拒否を結果として伝えるか、既存確定を先に成功させる契約を明示する。呼び出し側へ複数段階を露出しない。
3. 入力ラベルは単一 `TextInputState` の表示メタデータとする。履歴復元を含む入力対象の同期箇所で更新し、ラベル更新のために値やIME状態を再設定しない。
4. `select_title` に背景解除の責務を集約する。現コードは `canvas_editing` を解除していないため、その状態と単一mountを確認する。背景親に無条件の解除を付けず、文字編集・画像・列境界・ドラッグのイベント伝播を調べる。
5. astraの無効幅・外部競合・未知言語・日本語IMEの検証を採用する。solの長い一覧での選択露出に加え、キーボードで画面外の行へ移った際にスクロールも追従することを確認する。
6. 言語候補はハイライトの既存一覧から導出できるかを先に調べる。独立固定リストが単なる少数の推薦候補なら、その意味を明示し、対応言語の完全一覧として二重管理しない。

## GPUI・既存コードで確認した事実

- このcheckoutのGPUIには `Role::ListBox`、`Role::ListBoxOption`、`aria_selected` があり、`src/navigator.rs` でも使う。solのrole名は架空ではない。
- 通常の `ScrollHandle::scroll_to_item` は `index` 一つを受ける。GPUI `elements/div.rs:4316` で確認した。Navigatorの `scroll_to_item(index, ScrollStrategy::Nearest)` は別のスクロール型である。lunaの `scroll_to_item(Nearest)` 相当を通常Handleの署名として転記してはいけない。
- 通常Handleのスクロールは直接の子のboundsを参照するため、solの「行を直接の子とする」は根拠がある。ただし全行を通常divで生成しても仮想化にはならない。lunaの「可視化に任せる」を描画コストの制限として解釈しない。
- GPUIのdivはフォーカスがありclick listenerを持つ場合にEnter／Spaceをclickへ変換する。roleがButtonであることを条件にはしていない。ListBoxOptionへのrole変更自体で自動的にEnter／Spaceが失われるという断定は誤り。ただし `.focusable()`、`.tab_stop(...)` とAX Clickの接続は保持する。
- roleと `aria_selected` の設定だけでは矢印移動や画面外フォーカスの露出は実装されない。Navigatorのroleだけをコピーして操作も同じになると考えない。実機でTab・Enter・Space・VoiceOverを確認し、ListBoxを採るなら矢印移動の契約も定める。
- `select_current_field` は現在 `()` を返す。選択内部で拒否しても、その直後の `set_value` は呼び出し側では止められない。solの使用例はそのまま実装できない。

## Red flagsの判定

独立InspectorStateを退けた判断は全案で妥当。文書、履歴、IMEの二重所有を増やさない。

solのプリセット使用例は「呼び出し側が複数メソッドの順番を知る」というshallow moduleとtemporal decompositionに該当する。astraの単一適用入口へ置き換える。lunaの追加identity型は情報の重複、転送入口はpass-throughの懸念がある。solの `selected_source` と `selected_element` は既存同等処理があれば再利用し、同じEditTarget解釈を増やさない。公開API追加は入力ラベル設定にとどめ、Inspector用ヘルパーは非公開にする。

確認範囲は候補3件、grounding、red flags、既存選択・ボタン・Navigator、固定revのローカルGPUI。実装・実GUI操作は行っていない。
