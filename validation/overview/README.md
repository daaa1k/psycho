# Slide Overview の検証

表示メニューとツールバーから中央領域を Overview に切り替える。選択した Slide を並べ替え、Return またはダブルクリックで編集へ戻る。仕様の正本は GitHub Issues #48 と #54。

## 採用した構造

`EditorViewState.mode` が Edit と Overview を区別する。選択は既存の `PsychoApp.current_slide`、内容と履歴は既存の文書が所有する。Inspector の実効表示と幅をモードから導出し、ユーザーの表示設定と希望幅を保持する。Overview では中央キャンバスと Inspector を生成しない。

`navigator.rs` の collection は Navigator と Overview のカード、選択、AX 操作、ドラッグを共有する。各ホストは独立した `NavigatorState` を持ち、フォーカス、スクロール、表示寸法を分ける。格子の行を `uniform_list` で仮想化し、同じ geometry からカード幅、行数、上下移動、スクロール、挿入境界を決める。開始時に両ホストのドラッグ状態を消すため、同時に有効なドラッグは一つになる。

設計候補の `SlideCollections` ラッパーは追加せず、既存の状態型を二つ使った。ファイル名も維持した。別の選択・編集エンジン、Inspector 設定のコピー、文書操作を包む追加の層は導入しない。

ウィンドウのリサイズによる取消は、ドラッグ開始時のウィンドウ寸法と比較する。キャンバスの実測寸法は Fit と格子レイアウトに使い、フォーカス枠の変化ではドラッグを取り消さない。

macOS では GPUI の firstResponder と AccessKit が付く contentView が異なる。起動時に AccessKit の公開 focus forwarder を `GPUIWindow` に一度登録し、ネイティブのフォーカス問い合わせを選択項目へ渡す。直接依存は GPUI と同じ AccessKit 0.26.3 に固定する。GPUI の更新時には、クラス登録と転送の前提も再確認する。

モード切替は入力を確定してから行う。外部競合中に未確定入力がある場合は、文書への適用前に拒否して理由を示す。競合中でも未確定入力がなければ読み取り専用の切替を許す。既存の `prepare_view_change` の規則は変更しない。

## 再実行

macOS、接続ディスプレイ一つ、Orca の Accessibility と画面収録、Xcode CLI tools、Python の Pillow が必要。実行先は未使用の `target/` 配下にする。ユーザー文書を使わない。GUI ドライバーは一つだけ動かす。cargo test、ビルド、GUI 検証はこの順に実行する。実行中の再ビルドを避け、まとめて実行するスクリプトは各工程の前にバイナリのハッシュと doctor を確認する。

```sh
python3 -m venv target/overview-env
target/overview-env/bin/pip install Pillow
cargo fmt --check
CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang cargo test --locked --features gui
PSYCHO_BUILD_REVISION=$(git rev-parse HEAD) CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang cargo build --locked --features gui --bin psycho
.agents/skills/verify/helpers/psycho-verify doctor
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-main --revision <build-revision> --section main
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-resize --revision <build-revision> --section resize
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-drag --revision <build-revision> --section drag
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-guards --revision <build-revision> --section guards
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-empty --revision <build-revision> --section empty
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-invalid --revision <build-revision> --section invalid
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-rich --revision <build-revision> --section representative
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-light --revision <build-revision> --section layout --appearance light
target/overview-env/bin/python validation/overview/acceptance.py --output target/overview-dark --revision <build-revision> --section layout --appearance dark
target/overview-env/bin/python validation/overview/voiceover.py --output target/overview-voiceover --revision <build-revision>
xcrun swift validation/mvp-acceptance/with_japanese_ime.swift "$PWD/target/overview-env/bin/python" validation/overview/ime.py --output target/overview-ime --revision <build-revision>
```

`main` は表示設定、操作の無変更、選択、Return、ダブルクリック、ドラッグ取消、行をまたぐ移動、右クリック、キー操作、Undo/Redo、直接編集、保存、発表、復帰を照合する。`drag` は 100 枚と末尾の短い行で、静止ポインターの両端スクロール、モード切替による取消、古い原文のドラッグ取消を確認する。

`resize` は高さだけのウィンドウ変更でドラッグを取り消すことを確認する。`diagnostics` は Overview の欠落画像診断から該当 Slide と画像へ移動し、原文と履歴を変えずに Edit へ戻ることを確認する。

`guards` は無効な列幅・タイトルと外部競合中の未確定入力を確認する。`empty` は最初の追加、最後の削除、Undo を確認する。`invalid` は初回構文不正で診断だけを表示することを確認する。`representative` は全 Element、空 Slide、画像更新、外部再読込、最後の正常な表示を確認する。

`layout` は空 Slide、長い見出し、列内の本文・コード、欠落画像を含む 100 枚を使用する。1024×700、1280×860、1440×960 の各寸法で、列数、矢印、末尾到達、ペイン変更を照合する。`voiceover.py` は実際の発話と操作を記録する。AX 属性だけでは合格にしない。VoiceOver と IME の設定復元は製品 PID の生存に依存しない。

Overview と既存 Navigator・Slide 操作をまとめて再実行する場合は、次を使う。

```sh
target/overview-env/bin/python validation/overview/run.py --output target/overview-suite --revision <build-revision> --regressions
```

失敗した工程だけを新しい出力先で再実行する場合は、`--only invalid representative` のように工程名を指定する。既存機能の工程名を指定する場合は `--regressions` も付ける。各実行の結果を残し、未実行の工程を合格に数えない。

VoiceOver のレシピは、ユーティリティの「一般」設定がアクセシビリティへ公開されてから設定を読み取る。起動直後の固定待ち時間に依存しない。

VoiceOver 有効時は、各 Right で選択が1つ進んだことを確認してから次を送る。再送せず、97回の入力で3枚目から100枚目へ進む。実際の発話、ネイティブのフォーカス、両方向の並べ替え、Undo、Redo、編集への復帰を照合する。

高速な連続入力では遅延した AX Click が以前の項目を再選択する問題が残る。[#73](https://github.com/daaa1k/psycho/issues/73) で追跡する。通常のキー長押しでの再現有無は未確認。上の到達性検証を、この問題の解決や処理速度の証明には使わない。`voiceover.py` に `--burst` を付けると連続入力の再現条件を実行する。この診断用実行は通常スイートから分け、失敗を合格へ数えない。

## 証跡

採用した結果は [実機検証の記録](evidence/README.md) を参照する。実行ごとの PNG、操作記録、原文と保存後の KDL は指定した `target/` の出力先に残る。採用する代表画像と検証結果だけを `evidence/` に保存する。検証したソースのリビジョンと、後続の文書・証跡だけのコミットを区別する。失敗した試行は合格へ数えない。
