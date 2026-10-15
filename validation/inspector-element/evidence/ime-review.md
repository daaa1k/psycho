# IME 最終独立レビュー

判定は承認。`c3b2bdd` の `1e39042..HEAD` にある `src/ime.rs` と `src/main.rs` の差分について、修正を要求する具体的な問題は見つからなかった。取り消された AX bridge は対象外。GUI 起動、ビルド、製品ソース変更は実施していない。

`CompositionGeometry` は composition の元座標とパネルごとの相対位置を保持する。各 refresh は現在の marked start 座標から絶対位置を計算するため、重複した非同期通知で移動量が累積しない。既存パネルから registration origin を求める処理は、補助パネルの交換時に既存グループの移動と画面端補正を引き継ぐ。全パネルの frame を変更前に取得している。

`PanelAnchor` の `StrongPtr` は記録中のパネルの解放とポインタ再利用を防ぐ。表示対象から消えたパネルは retain 処理で記録から除かれる。`setFrame` と `invalidateCharacterCoordinates` の前に `RefCell` の可変 borrow は解放される。foreground task に await はなく、この処理内で非同期通知が交互に実行される箇所はない。

GPUI `14dd03e/crates/gpui_macos/src/window.rs` の `has_marked_text`、`marked_range`、`first_rect_for_character_range`、`with_input_handler` を確認した。`hasMarkedText` は marked range の存在を返し、入力ハンドラ不在では false になる。false の分岐では座標問い合わせも候補パネル移動も行わず、保存状態を破棄する。`src/input.rs` の確定処理は marked range を消去して notify する。今回の無条件 editor observer と blur callback は、その状態を描画後に読み直す経路を持つ。

検証コード `validation/inspector-element/marked_keys.py` の `same_panels` はパネル数が等しく、少なくとも 2 個あることを確認して両パネルの X/Y を比較する。同じ composition の Tab、Shift+Tab、canvas 往復は 3px 以内のまま。別 composition との比較だけを 8px にする変更は、主候補と補助候補の大幅な位置ずれを隠さない。親エージェントが報告した 4px 差の原因をフォント配置と断定する証拠は、このレビューでは得ていない。

`/tmp/psycho53-ime-absolute-anchor.log` と `/tmp/psycho53-ime-geometry-final.log` は最初の composition の各段階の PASS を含む。第二 composition の最終実行結果は、このレビューの独立した実測結果には含めない。原則 Prove It Works に従い、ソース照合と読み取ったログの結果を、親エージェントによる実行報告と区別した。

コメントは AppKit の再入、候補パネルのレベル、補助パネルの追従という外部挙動の理由を説明している。コメント削除、復元、コード修正は 0 件。指示どおり追加エージェントは起動していない。
