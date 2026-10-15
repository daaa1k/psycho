# Issue 53 設計候補

## 問題

既存の `EditTarget`、単一の `TextInputState`、`commit_draft` が選択と編集中の値、検証、IME、履歴をすでに扱っている。Issue 53ではこの所有権を保ち、Inspectorを対象別に整理する。変更範囲は `src/main.rs` の表示・既存選択入口と `src/input.rs` の入力メタデータ、GUI受け入れ検証。新しい編集モデルは不要。

## 呼出側から見た使い方

構造一覧の行とCanvasは引き続き同じ入口を使う。選択されている行は `EditTarget` から導く。

```rust
// 構造一覧の行。Canvasも同じ既存入口を使う。
this.select_canvas_source(source, window, cx);

// 背景の空白でのみ実行。既存選択入口で入力確定する。
this.select_title(window, cx);

// コード設定の候補。任意入力は既存のCodeLanguage欄を使用する。
this.apply_code_language("rust", window, cx);
```

未選択のInspectorは「プレゼンテーション」のタイトル入力と「スライドの構造」を表示する。選択すると先頭に「本文」「画像」「コード」「2列」などの対象名、続いて対象の設定と全文編集、移動・削除、最後に構造一覧を表示する。構造一覧だけを操作してもCanvasの表示枠と選択が一致する。

## 型と責務

新しい永続的な選択型やInspectorモデルを置かず、既存 `EditTarget` とモデルの照合を表示時に行う。現在ある `current_element_source()` 相当の既存取得処理を優先再利用し、Outline独自の選択照合を重複させない。

```rust
impl PsychoApp {
    // 既存のrender_inspectorが対象解決と対象別描画を一か所で扱う。
    fn render_inspector(
        &mut self, model: Option<&PresentationModel>, cx: &mut Context<Self>
    ) -> gpui::AnyElement { unimplemented!() }

    // 全件を描画。ページ状態を持たない。
    fn render_element_outline(
        &mut self, model: &PresentationModel, cx: &mut Context<Self>
    ) -> gpui::AnyElement { unimplemented!() }

    // 既存選択と確定の手順をここで完結させる。
    fn apply_code_language(
        &mut self, language: &str, window: &mut Window, cx: &mut Context<Self>
    ) { unimplemented!() }
}

impl TextInputState {
    // 値・選択範囲・IME compositionを変更しない表示メタデータ。
    pub fn set_accessibility_label(
        &mut self, label: SharedString, cx: &mut Context<Self>
    ) { unimplemented!() }
}
```

`apply_code_language` は外部競合の既存判定、現在のdraft確定、既存の `CodeLanguage` 選択、editorへの候補設定、既存確定の順に実行する。前段で拒否されたら後段へ進まない。既存 `select_current_field` が成功を返さない場合、この新規用途のために既存メソッドを結果返却へ変更するか、確定後のターゲット確認を行う。選択失敗を無視して別欄に言語を書き込む実装は不可。言語候補は既存のハイライト対応一覧から取得できるならそれを用い、独立した同義一覧を増やさない。候補は値の入力補助であり、未知の文字列を読み込みや再描画で書き換えない。

入力ラベルは対象変更・欄変更・履歴復元など、既存editorの内容を対象へ合わせる箇所で併せて設定する。幅は「左列幅、1から99の整数」、言語は「コード言語、任意の文字列」、画像は既存パス規則を説明する。ラベル更新のために `set_value` を呼ばない。CanvasとInspectorは同じeditorを一度だけmountし、`canvas_editing` による切替とIME geometry更新を維持する。

## スクロールと到達性

Inspector全体の既存 `overflow_y_scroll` をスクロールの所有者とする。Outlineの `max_h(180)`、`PAGE_SIZE`、`OutlinePage` とページ送りを削除し、全行を通常の高さで並べる。これで長い箇条書きや移動先一覧も同じスクロールで到達でき、入れ子の二重スクロールを避ける。先頭の対象設定を使うために、Canvasまたは構造一覧から対象が変わった際はInspectorのスクロールを先頭へ戻す。再描画、IME入力、幅変更では戻さない。

この案は構造一覧から選択すると位置が先頭へ移ることを受け入れ、選んだ対象の設定が確実に見えることを優先する。長い構造から連続して選ぶ操作では戻りスクロールが必要になる。これがGUI検証で不便なら、対象設定と構造一覧を別のスクロール領域へ分ける案へ変更する。先にスクロール状態の保存マップや仮想化は導入しない。

## 背景クリックとアクセシビリティ

Canvas背景のハンドラはElement、文字編集、列境界、ドラッグからのイベントを背景と誤認しない必要がある。既存のhit-test／ターゲット判定で空白を判別できればそれを使う。親コンテナへ無条件の `select_title` を追加しない。背景解除も `commit_draft` の拒否時には元の対象・入力を保持する。`select_title` の成功時に `canvas_editing` が解除されるかを確認し、抜けがあればこの既存入口へ修正を集約する。

行は従来の✓表示に加え、GPUIが対応する意味的な選択状態を公開する。標準の選択可能な行roleと `aria_selected` の対応をローカル実装で確認し、roleだけ変えてキーボード操作が消える設計にはしない。全行と設定操作をTabから辿れるようにし、フォーカスされた画面外要素が可視領域へ入ることも実機で確認する。入力制約は画面にも表示する。無効入力の既存statusを残し、「入力を取り消す」は `cancel_input` に接続する。外部競合時の編集停止表示は設定の通常描画より優先する。

## 採用理由と別案

この案では文書所有者に選択処理を残す。呼出側は既存の選択メソッドを一度呼べばよく、入力確定、履歴、IMEを知る必要がない。新規の公開APIは入力ラベルのみで、候補適用はPsychoApp内の非公開操作である。boundary-disciplineとsingle source of truthを優先する。

構造的に異なる案は `InspectorState` を独立Entityにし、`InspectorSelection` と属性ごとのdraftを所有させ、文書操作を親へ通知する方式。部品単体の描画とスクロールを局所化できる一方、Canvasと選択の同期、単一editorの受け渡し、通知前のIME確定、Undo後の再同期を公開契約に増やす。Issue 53の規模では利用側へ漏れる複雑さが大きいため採用しない。

## 検証と未確定事項

長い構造の末尾、左右列内、画面外本文から選択・全文編集・Undo・保存・再読込を実画面で行う。未知言語は開閉・プリセット表示のみで変わらず、候補適用はUndoで戻ることを確認する。幅の無効値で背景解除・別行選択・Inspector非表示を試し、入力保持と取消を確認する。CanvasとInspector往復時に標準日本語IMEの変換文字と候補位置を確認する。VoiceOverで行の役割・選択・入力制約を読み、Tabで画面外行を選べることを確認する。スクリーンショット、KDL差分、文字配置traceを残す。

GPUIの選択roleと自動フォーカススクロールは実装確認が必要。未対応なら既存アクセシビリティ接続を狭く拡張する。未知のAPI名を前提に実装完了としない。

## 統合判断と実装整合

候補段階につき統合判断は親agentが行う。実装差分は未作成。最初の実装単位はOutlineの全件化とページ状態削除、既存選択共有を保つGUI確認とする。
