# Transplanter

※現在は、Windows専用です。

Transplanterは、Steamゲーム [農家は Replace() されました](https://store.steampowered.com/app/2060160/_Replace/?l=japanese) で、Pythonだけでなく別の言語でもプログラミング学習を楽しむための非公式変換アプリです。

現在入っている変換器は次の2つです。

- `Rust -> ゲーム用Python`
- `Scheme風Lisp -> ゲーム用Python`

今後も変換器を増やしていく予定です。お楽しみに。

## 入手と初回セットアップ

Transplanterは、CursorやVS CodeなどのIDEでコードを書き、ゲーム側のファイルウォッチャーで反映する遊び方を想定しています。

そのためまずは、ゲーム側で外部エディタを使える状態にします。

1. [農家は Replace() されました](https://store.steampowered.com/app/2060160/_Replace/?l=japanese) を起動します。
2. `設定 / ファイルウォッチャー` を有効にします。
3. `Load` メニューを開きます。
4. 使いたいセーブを選びます。
5. `Open Folder` を押して、ゲームの Save フォルダを開きます。

次に、Transplanterを導入する作業です。

1. `C:\Users\YourName\Desktop\farming` のような、好きな作業フォルダを作ります。
2. [GitHub Releases](https://github.com/Aeniminu/Transplanter/releases) の最新リリースを開きます。
3. `Assets` から `Transplanter.exe` をダウンロードし、その作業フォルダへ置きます。
4. `Transplanter.exe` をダブルクリックします。
5. (自動)初回起動で必要なファイルが自動作成されます。
6. (自動)ウィンドウ上段の `ソースフォルダ` が、作業フォルダ内の `play_src` を指していることを確認します。
7. ウィンドウ下段の `ゲームの Save フォルダ` に、さきほどゲームから開いた Save フォルダを指定します。
8. 言語モードを選びます。初期設定は `Rust` です。
9. 自動生成されている `play_src/main.rs` を編集して保存し、左上の実行ボタンを押します。

うまく設定できていれば、Save フォルダ内の `main.py` が更新され、実行中は保存のたびにゲーム側へ反映されます。後はご自由にお楽しみください。

## Save フォルダの見つけ方

ゲーム側で `Open Folder` を押して開いた場所が指定先です。開いた場所に `__builtins__.py` や既存の `.py` が見えていれば、そこが正解です。

`Saves` という親フォルダではなく、実際に `.py` が入っているセーブ個別のフォルダを指定してください。

```text
Saves/Save0
Saves/Save1
```

## どう動くか

下の図のように、作業フォルダの中で書いたコードが、ゲームの Save フォルダへ `.py` として出力される単純な仕組みです。

```text
好きな作業フォルダ/
  Transplanter.exe
  .transplanter/
    transplanter.toml
    Cargo.toml
    transplanter_rust/
  play_src/
    main.rs
    code0.rs

        ↓ 変換

ゲームのSaveフォルダ/
  __builtins__.py
  main.py
  code0.py
```

`play_src` はあなたが書く場所です。Rust専用ではないので、言語モードを `Lisp` にすると `.scm` / `.lisp` も使えます。ゲームが読むのは Save フォルダ内の `.py` です。

`.transplanter` はTransplanterの設定やIDE補助をまとめる隠しフォルダです。普段触る場所は `play_src` だけ、と考えて大丈夫です。

言語モードはセーブごとに選べます。`Rust` では `.rs` だけ、`Lisp` では `.scm` / `.lisp` だけを変換します。`自動` は上級者向けの混在モードで、`.rs` / `.scm` / `.lisp` を拡張子から判定して変換します。

Rust / Lisp を切り替えた時、Transplanterが自動生成した未編集の `main.rs` / `main.scm` やRust補助ファイルは選択中の言語に合わせて整理されます。自分で編集したコードは削除しません。

## 最初の main.rs

初回起動で作られる `play_src/main.rs` は、次のようなRustコードとして編集できます。

```rust
use transplanter_rust::prelude::*;

fn main() {
    harvest();
}
```

生成される `main.py` はこのような形です。

```python
harvest()
```

`use transplanter_rust::prelude::*;` は、IDEでゲーム用関数を見つけやすくするための行です。変換後の `.py` には出力されません。

## Lisp も使う場合

言語モードを `Lisp` にすると、`play_src/main.scm` が自動作成されます。`.scm` または `.lisp` を `play_src` に置くと、同じように `.py` へ変換できます。

```scheme
(use transplanter)

(define (main)
  (harvest))
```

Lisp版では、上のように丸かっこで命令を書きます。`main` がゲームに送るコードの入口です。関数名は Lisp らしく `quick-print` のように書け、変換後はゲーム用Pythonの `quick_print` になります。方向やアイテムなどは `:east`、`(entity bush)`、`(item fertilizer)` のように書けます。

たとえば、東へ移動して木を植えるなら次のように書けます。

```scheme
(use transplanter)

(define (main)
  (move :east)
  (plant (entity tree)))
```

Lisp版を使う場合は、Transplanter本体に加えて Guile Scheme または Chez Scheme が必要です。これは `.py` を作る前に、Lispコードとして大きな書き間違いがないか確認するためです。入っていない場合、Rust版だけでも遊べます。

Guile / Chez が見つからない時やLispコードに誤りがある時は、ウィンドウ上に `error = "..."` として理由が表示され、対応する `.py` は更新されません。

## アップデート

Transplanter は起動時に GitHub Releases の最新版を確認します。今使っている exe より新しいリリースがある場合だけ、ウィンドウ上部に更新ボタンが表示されます。

手動で更新する場合は、作業フォルダ内の `Transplanter.exe` を最新リリースの `Transplanter.exe` に差し替えてください。

## うまく動かないとき

Saveフォルダが空欄の間は、変換監視を開始しません。まずウィンドウ下段にゲームの Save フォルダを指定してください。

古いバージョンから `rs_src` を使っている場合は、起動時に中身を `play_src` へ移し、設定も `.transplanter/transplanter.toml` へ移行します。同名ファイルが衝突する場合は、ユーザーのコードを消さずに `error = "..."` で知らせます。

起動時にはフォルダ構成も確認します。古い `Cargo.toml`、`Cargo.lock`、`.transplanter_ide` など、Transplanterが生成したと判定できる旧補助ファイルだけを整理し、ユーザーが作ったコードは削除しません。

`.rs` / `.scm` / `.lisp` にエラーがある場合、対応する `.py` は更新されません。ゲーム側へ壊れたコードを流さないためです。

`自動` モードでは、同じ出力名になるファイルは同時に置けません。たとえば `main.rs` と `main.scm` はどちらも `main.py` になるため、片方だけにしてください。`Rust` / `Lisp` モードでは、選んでいない言語のファイルは無視されます。

## 詳しい仕様

構文、変換ルール、ゲームAPI対応表、コマンド利用、開発者向けの構成は [docs/technical-reference.md](docs/technical-reference.md) にまとめています。
