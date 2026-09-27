# PSYCHO 開発 引き継ぎ文書

## 1. プロジェクト概要

Rust + GPUI を使い、Windows / macOS 向けの新しいプレゼンテーションソフトを開発する構想。

単なる PowerPoint / Keynote の代替ではなく、以下の思想を中心に据える。

> **GUI-first だが、データは plain text を source of truth とするプレゼンテーション環境**

『達人プログラマー』的な「知識を proprietary binary format に閉じ込めず、長寿命で汎用的なプレーンテキストとして保持する」という考えを重視する。

Markdown を手書きすること自体は目的ではない。

---

# 2. 現在の主要要件

当初の要件は以下。

- Rust + GPUI
- Windows / macOS 対応
- リアルタイムプレビュー
- プレビュー画面上で WYSIWYG 編集
- WSL2 上のファイルを直接開いて編集可能
- 画像対応
- 動画対応
- Speaker Notes
- Presentation Mode
- Presenter View
- ポインター
- カンニングペーパー / Cue
- 一般的なプレゼンソフトの基本機能
- 高速なビルド
- 依存ライブラリは極力少なくする
- ただし画像処理・動画処理など複雑な領域は再発明しない
- Git と透過的に連携
- Plain text を正本とする

---

# 3. プロダクトの中心思想

最も重要な原則。

## Plain text is the source of truth

GUI 上の状態をバイナリや SQLite 等だけに保存するのではなく、プレゼンテーションそのものが普通の UTF-8 テキストとして存在する。

その結果、

```bash
git diff
rg
grep
sed
awk
vim
code
```

など普通の開発ツールから扱える。

また、

- GitHub 上でも読める
- 10年後も解析可能
- CLI から生成・変換可能
- CI から検証可能
- 特定アプリがなくても内容を確認できる

という性質を持たせる。

---

# 4. Markdown / MDX に関する検討結果

当初は Markdown をメインフォーマットにする案を検討した。

その後、MDX にすると以下が表現しやすいことを確認した。

```mdx
<Columns>
  <Column>
    ## Architecture
  </Column>

  <Column>
    <Image src="./architecture.png" />
  </Column>
</Columns>
```

ただし Full MDX を採用すると、

```mdx
{condition ? <Foo /> : <Bar />}
```

や、

```mdx
<Chart data={sales.filter(...)} />
```

など JavaScript の実行時意味論が Document Model に入り込む。

これは、

- WYSIWYG
- deterministic rendering
- Rust-only implementation
- CLI / PDF rendering
- source mapping

との相性が悪い。

一時は「JavaScript を排除した MDX-like declarative components」が候補になった。

しかし最終的には、

> Markdown に拘る必要はなく、Plain Text の方が本質

という結論になった。

---

# 5. 現在有力なファイルフォーマット: KDL 2.0

現在は **KDL をほぼそのままストレージ形式として利用する案が最有力**。

独自 DSL を作るのではなく、

> KDL 2.0 + Presentation Schema

とする。

KDL 自体を fork したり独自構文を追加しない。

## 例

```kdl
presentation {
    metadata {
        title "Rust + GPUI"
        theme "midnight"
    }

    slide id="intro" {
        heading "Building a Presentation App"
        subtitle "Rust / GPUI / Plain Text"

        notes """
        最初に、このツールを作った理由を説明する。
        """
    }

    slide id="architecture" {
        columns {
            column width=40 {
                heading "Architecture"

                bullets {
                    item "Plain text"
                    item "Source-aware AST"
                    item "GPUI renderer"
                }
            }

            column width=60 {
                image "assets/architecture.png" fit="contain"
            }
        }
    }
}
```

---

# 6. KDL の利用ルール案

KDL の構造をそのまま Presentation Model に活用する。

## Argument

主となる値。

```kdl
heading "Architecture"

image "assets/logo.svg"

video "assets/demo.mp4"
```

## Property

設定や属性。

```kdl
image "assets/logo.svg" width="60%" fit="contain"

video "demo.mp4" autoplay=#true
```

## Children

構造。

```kdl
columns {
    column {
        // ...
    }

    column {
        // ...
    }
}
```

---

# 7. Markdown の位置付け

Markdown を完全に捨てる必要はない。

Presentation の構造には KDL を使い、文章を書く用途では Markdown を leaf node として使う案。

```kdl
slide {
    markdown """
    ## Why Rust?

    - **Fast**
    - Memory safe
    - Great tooling
    """
}
```

役割分担は以下。

```text
KDL
    Presentation structure

Markdown
    Optional rich text representation
```

---

# 8. GUI とソースの関係

WYSIWYG Editor の本質は、

> **KDL AST の Visual Editor**

とする。

例えば GUI で画像幅を変更した場合、

```kdl
image "architecture.png" width="70%"
```

から、

```kdl
image "architecture.png" width="60%"
```

だけを書き換える。

---

# 9. Round-trip fidelity

極めて重要な要件。

## 原則1

> ファイルを開いて保存しただけでは内容を変更しない。

## 原則2

> GUI 操作は必要最小限の source diff だけを生成する。

例えば、

```diff
 image "logo.png" {
-    width "70%"
+    width "60%"
 }
```

程度にする。

Presentation 全体を再 serialize して、

```diff
- 200 lines
+ 200 lines
```

となる実装は避ける。

---

# 10. Lossless Syntax Tree

KDL AST は単に semantic data として扱うだけでなく、

- whitespace
- blank lines
- comments
- formatting
- property ordering

などを可能な限り保持する。

Compiler の Lossless Syntax Tree に近い設計を想定。

内部では、

```rust
struct Node {
    span: SourceRange,
    ...
}
```

のような source location を保持する。

GUI からの編集は semantic AST 全体を serialize するのではなく、source patch を生成する。

---

# 11. Formatter

保存時に formatter を自動実行しない。

```text
Save
    preserve original formatting

Format Document
    explicit user command
```

とする。

理由は Git diff の品質を維持するため。

---

# 12. コメント

コメントは一級市民として保持する。

```kdl
slide {
    // TODO: 最新のデータに更新する

    image "benchmark.png"
}
```

GUI から編集してもコメントを失わない。

Presentation source は「アプリの内部データ」ではなく、

> 人間とツールが共同編集できるソースコード

として扱う。

---

# 13. Presentation Schema

KDL と Presentation の意味論は分離する。

```text
KDL parser
    ↓
Syntax Tree
    ↓
Presentation Semantic Validator
    ↓
Typed Presentation Model
```

例えば、

```kdl
image "foo.png" autoplay=#true
```

は KDL として合法だが Presentation Schema としてはエラー。

```text
property `autoplay` is not valid for `image`
```

と診断する。

---

# 14. Schema-driven UI

Presentation Schema 自体を KDL で記述する案もある。

例:

```kdl
component "image" {
    argument "src" type="path" required=#true

    property "width" type="length"
    property "height" type="length"

    property "fit" enum="contain cover fill"
}
```

これを Single Source of Truth として、

```text
Schema
 ↓
Validation
 ↓
Inspector UI
 ↓
Documentation
```

を生成する。

例えば、

- `bool` → checkbox
- `enum` → select
- `length` → numeric / unit editor
- `color` → color picker

など。

---

# 15. Rust 内部モデル

KDL AST と Presentation Domain Model は分離する。

例えば、

```rust
struct Presentation {
    slides: Vec<Slide>,
    theme: Theme,
    metadata: Metadata,
}
```

```rust
struct Slide {
    id: Option<String>,
    children: Vec<SlideNode>,
}
```

```rust
enum SlideNode {
    Heading(Heading),
    Text(Text),
    Markdown(Markdown),
    Image(Image),
    Video(Video),
    Columns(Columns),
    Code(Code),
    Note(Note),
    Cue(Cue),
}
```

Presentation Model は GPUI に依存させない。

---

# 16. Architecture

現時点の想定。

```text
app
│
├── presentation_core
│     Presentation
│     Slide
│     Element
│     Theme
│     Layout
│
├── source
│     KDL parser
│     SourceMap
│     Lossless AST
│     Patch
│
├── schema
│     Validation
│     Component definitions
│
├── editor
│     Canvas editor
│     Source editor
│     Selection
│     Undo / Redo
│
├── renderer
│     LayoutEngine
│     PresentationRenderer
│
├── media
│     Image
│     Video
│
├── presenter
│     PresenterView
│     Pointer
│     Notes
│     Cue
│     Timer
│
├── workspace
│     FileSystem
│     WSL
│     Assets
│
├── git
│     Status
│     Commit
│     Push
│
└── platform
      macOS
      Windows
```

重要:

```text
presentation_core
```

は GPUI を知らないこと。

---

# 17. GPUI

UI Framework は GPUI を使用する予定。

理由:

- Rust native
- GPU accelerated
- Windows / macOS 対応
- Zed と同系統の UI architecture
- Electron を避けられる

ただし GPUI はまだ比較的変化が大きいため、Presentation Core と強く結合させない。

---

# 18. Editor UI

想定 UI。

```text
┌──────────────┬───────────────────────────┬───────────────┐
│ Slides       │                           │ Inspector     │
│              │                           │               │
│ 01 Intro     │          Canvas           │ Layout        │
│ 02 Problem   │                           │ Typography    │
│ 03 Design    │       WYSIWYG View        │ Background    │
│ 04 Demo      │                           │ Animation     │
│              │                           │               │
├──────────────┴───────────────────────────┴───────────────┤
│ Source / Problems / Git / Assets                        │
└──────────────────────────────────────────────────────────┘
```

Editor Mode:

```text
Canvas
Source
Split
```

を想定。

Plain Text を重視するが、

> ユーザーが source を触る必要はない

という UX を目指す。

---

# 19. Presentation Mode

最低限欲しい機能。

- Current slide
- Next slide
- Speaker Notes
- Cue
- Elapsed timer
- Countdown timer
- Slide number
- Laser pointer
- Spotlight
- Annotation
- Cursor hide
- Black screen
- White screen
- Freeze audience display
- Jump to slide
- Multi-monitor
- Presentation output selection

---

# 20. Cue / カンニングペーパー

Speaker Notes と別に Cue を持つ案。

Speaker Notes は文章。

Cue は発表時に見る短い talking points。

```kdl
cue {
    item "GPUI ≠ renderer"
    item "AST と render tree を分離"
    item "動画は OS decoder"
}
```

Presenter View では、

```text
NEXT TALKING POINT

→ GPUI ≠ renderer
→ AST と render tree を分離
→ 動画は OS decoder
```

のように表示する。

---

# 21. Image / Video

画像は普通の asset file として保持。

```kdl
image "assets/architecture.png"
```

Clipboard から貼り付けた場合、

```text
assets/
    8ac3c921.png
```

等として保存して source を更新する。

動画については OS native backend を検討。

```text
MediaPlayer
  ├─ macOS
  │    AVFoundation
  │
  └─ Windows
       Media Foundation
```

FFmpeg 等を全面依存として入れることはできるだけ避ける。

---

# 22. Dependency Philosophy

「完全 zero dependency」ではなく、

> **Dependency Budget**

という考えを採る。

複雑で成熟した問題は再発明しない。

ただし巨大 dependency graph は避ける。

特に、

- Markdown parser
- KDL parser
- image decoder
- native video APIs

などは既存実装を使うことを検討する。

---

# 23. Git

Git は重要な差別化要素。

ユーザーから見ると、

```text
✓ Saved
↑ Synced
```

程度の UI を想定。

初期段階では Git library を組み込むより、

```bash
git
```

CLI を呼び出す案が有力。

理由:

- SSH agent
- credential helper
- GPG signing
- user config

などをそのまま利用可能。

自動 commit / push を行う場合も、Presentation 関連ファイルだけを pathspec で対象にする。

同一 repository の unrelated files を勝手に commit しない。

---

# 24. WSL2

Windows 版では WSL2 内の Presentation を直接編集できるようにする。

UI のイメージ:

```text
Open

Local

WSL
 ├─ Ubuntu
 ├─ Ubuntu-24.04
 └─ Debian
```

ただし WSL filesystem への高頻度 file I/O は避ける。

基本:

```text
File
 ↓
Memory Buffer
 ↓
Edit / Incremental Parse / Render
 ↓
Debounced Atomic Save
```

毎キー入力ごとに filesystem を read/write しない。

---

# 25. Layout

Semantic layout を重視。

PowerPoint のような完全自由配置を最初から目指すと source representation が複雑になるため、初期は preset / semantic layout を中心にする。

例:

```text
Title
Title + Content
Columns
Image + Text
Full Image
Quote
Code
Comparison
Section Divider
```

KDL なら、

```kdl
columns {
    column width=40 {
        // ...
    }

    column width=60 {
        // ...
    }
}
```

のように表現する。

Absolute positioning は必要になってから追加する。

---

# 26. Theme

各 element が直接 font-size 等を持ちすぎないようにする。

Semantic Tokens:

```text
Title
Subtitle
Heading
Body
Caption
Code
```

Theme がこれらを定義する。

```text
Theme
 ├── Typography
 ├── Colors
 ├── Spacing
 ├── Background
 ├── Layout presets
 └── Code theme
```

---

# 27. Developer-oriented Features

このアプリのターゲットと相性が良い機能。

## Code block

```kdl
code "src/main.rs" language="rust"
```

## External source reference

```kdl
code "../app/src/main.rs" {
    lines 12..32
}
```

Presentation にコードをコピーせず、実際の source file を参照できるようにする。

コード更新時に Presentation も自動更新可能。

## Code Steps

```kdl
code "src/main.rs" {
    step lines="1..8"
    step lines="12..18"
    step lines="20..32"
}
```

---

# 28. Presentation Lint

IDE 的な価値を出せる機能。

例:

```text
Problems

⚠ Slide 12
  Text may be too small.

⚠ Slide 18
  14 bullet points.

⚠ Slide 21
  Low-resolution image.

⚠ Slide 28
  Video file not found.

⚠ Slide 32
  Low contrast.
```

将来的には、

- Overflow
- Missing asset
- Broken URL
- Unsupported codec
- Low contrast
- Font substitution
- Estimated speaking time

等を診断する。

---

# 29. CLI

Presentation Core が GPUI 非依存なら、headless CLI が作れる。

例:

```bash
psycho presentation.kdl --check

psycho presentation.kdl --pdf

psycho presentation.kdl \
    --slide architecture \
    --png
```

CI / GitHub Actions でも利用可能。

CLI 名は未決定。

---

# 30. PDF Export

Chromium / WebView を使って HTML → PDF にする方式は極力避けたい。

理想:

```text
Presentation Model
        ↓
Layout Engine
        ↓
Display List
      ↙     ↘
    GPUI    PDF
```

Screen / PDF が同じ layout result を共有する。

---

# 31. Stable IDs

重要な slide だけ semantic ID を持たせる案。

```kdl
slide id="architecture" {
}
```

用途:

- internal links
- Presenter navigation
- Git history
- URLs
- Remote control

すべての element に UUID を埋め込むのは source が汚くなるので避ける。

---

# 32. 用語

`deck` という語は既存 OSS との衝突・連想があるため使用を避けたい。

現在の基本語彙:

```text
Presentation
Slide
Element
Theme
Asset
Notes
Cue
Transition
```

内部実装では Render Unit を `Scene` とする可能性もあるが、ユーザー向けには普通に `Slide` を使う方針。

トップレベル KDL node は、

```kdl
presentation {
}
```

が有力。

ファイル名候補:

```text
presentation.kdl
talk.kdl
slides.kdl
conference-talk.kdl
```

独自拡張子を無理に作らず `.kdl` をそのまま使う案が有力。

---

# 33. アプリ名

アプリ名は、

# PSYCHO

を予定。

頭字語として意味を持たせたい。

現在有力候補。

## Candidate A

**Presentation Software You Can Hack Openly**

最も自然で説明しやすい。

## Candidate B

**Presentation System: Your Content, Hackable & Open**

プロダクト思想を最もよく表している。

```text
PSYCHO

Presentation System:
Your Content,
Hackable & Open
```

## Candidate C

**Presentation System You Can Hack & Own**

Ownership / Plain Text の思想が強い。

現時点では、

> **Presentation System: Your Content, Hackable & Open**

が特に好感触。

---

# 34. PSYCHO の Positioning

一言で表現すると、

> **GUI-first, source-controlled presentation software.**

あるいは、

> **A native presentation tool where your presentation remains plain text.**

方向性としては、

```text
                Source-oriented
                      ↑

       LaTeX          │          PSYCHO
       Marp           │             ●
                      │
──────────────────────┼──────────────→ Visual editing
                      │
                      │
                  PowerPoint
                   Keynote
```

を狙う。

つまり、

> Presentation-as-Code の利点を持ちながら、コードを書かなくても使える。

という立ち位置。

---

# 35. 現在の設計原則

今後の判断では以下を優先する。

1. **Plain text is the source of truth**
2. **GUI と source は完全に round-trip する**
3. **保存時に不要な diff を作らない**
4. **標準 OS / Git / CLI tools から扱える**
5. **Presentation Model と UI Framework を分離する**
6. **KDL 自体は fork / 拡張しない**
7. **Presentation 固有意味論は Schema として定義する**
8. **GUI を使うために source を理解する必要はない**
9. **Source を直接編集しても GUI が壊れない**
10. **複雑な既存技術を無意味に再実装しない**

---

# 36. MVP 案

## Phase 1

```text
KDL parsing
Presentation AST
Basic slide rendering
Text
Image
Live preview
Presentation mode
```

## Phase 2

```text
Canvas editing
Source editing
Split view
Lossless source patching
Undo / Redo
Layouts
Theme
```

## Phase 3

```text
Presenter View
Notes
Cue
Multi-monitor
Pointer
Fragments
PDF
```

## Phase 4

```text
WSL
Git integration
Video
Presentation lint
CLI
```

---

# 37. 次に検討すべきテーマ

次の担当者には、以下を優先して詰めてもらいたい。

## 最優先: Presentation KDL v0.1 の仕様

最低限、

```text
presentation
metadata
slide
heading
text
markdown
image
video
columns
column
notes
cue
code
```

について、

- arguments
- properties
- children
- required / optional
- validation rules

を定義する。

---

## 次点: Source architecture

以下を具体化する。

```text
KDL Source
 ↓
Lossless Syntax Tree
 ↓
Semantic Presentation AST
 ↓
Layout Tree
 ↓
Display List
 ↓
GPUI
```

特に、

```text
GUI edit
 ↓
Semantic edit
 ↓
Source patch
```

の仕組みが技術的な最重要ポイント。

---

## 次点: WYSIWYG の編集境界

「Semantic WYSIWYG」をどこまで許すか決める。

初期段階では、

- text edit
- image resize
- reorder
- columns resize
- layout change
- theme change

程度。

完全自由配置は後回しにする。

---

## 次点: Schema

Presentation Schema を Rust code で定義するか、KDL schema file から生成するか検討する。

理想は、

```text
Schema
 ↓
Parser validation
 ↓
Rust model
 ↓
Inspector
 ↓
Docs
```

を統一すること。

---

# 38. 未決定事項

以下はまだ決まっていない。

- PSYCHO の正式な acronym
- KDL file naming convention
- Markdown block を標準機能にするか
- Presentation Schema の実装方式
- Layout Engine の具体設計
- Text shaping / typography architecture
- Undo / Redo と source patch の関係
- Plugin system
- Extension component の仕組み
- Animation / timeline syntax
- Absolute positioning の導入時期
- PDF backend
- Video backend abstraction
- Git sync policy
- Asset naming / content-addressing
- CLI executable 名

---

# 39. 次回の会話開始時に推奨する議題

次は、

> **「Presentation KDL v0.1 を実際に設計する」**

ところから始めるのが最も自然。

具体的には、10枚程度の実用的な Presentation を KDL で書いてみて、

- タイトル
- 箇条書き
- Columns
- Image
- Code
- Video
- Notes
- Cue
- Fragments

を表現する。

そのサンプルから逆算して、

```rust
Presentation
Slide
Element
```

の Rust 型と schema を決める。

これは syntax-first ではなく、

> **実際に人間が使う Presentation を先に書き、それを最小限の Schema に落とす**

という進め方が適している。
