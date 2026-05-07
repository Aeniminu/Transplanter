# farmrs

The Farmer Was Replaced 向けの Rust 風 DSL トランスパイラです。

## これは何か

`farmrs` は `.farmrs` ファイルを、ゲームで利用できる Python 風の `.py` ファイルに変換します。

## これは何ではないか

これは Rust コンパイラではありません。所有権、借用、ライフタイム、trait、macro、module、crate、Rust 標準ライブラリは実装しません。

## 導入方法

### 実行ファイルをダウンロードして使う場合

配布済みの実行ファイルがある場合は、Rust や Cargo をインストールせずに使えます。

1. Releases から自分の環境に合う実行ファイルをダウンロードします。
2. 必要に応じて、実行ファイルを PATH が通っている場所に置きます。
3. 次のコマンドで動作確認します。

Windows の場合:

1. Release の Downloads から `farmrs-windows-x86_64.exe` をダウンロードします。
2. 使いやすいように、必要ならファイル名を `farmrs.exe` に変更します。
3. PowerShell で動作確認します。

```powershell
.\farmrs.exe --help
```

macOS / Linux の場合:

```bash
chmod +x farmrs
./farmrs --help
```

手元の `.farmrs` ファイルを変換する場合:

```bash
farmrs my_script.farmrs -o my_script.py
```

Windows で PATH に置かず、カレントディレクトリから直接実行する場合:

```powershell
.\farmrs.exe my_script.farmrs -o my_script.py
```

このリポジトリ内のサンプルを変換する場合:

```bash
farmrs examples/basic.farmrs -o output.py
```

### ソースからビルドする場合

ソースからビルドする場合は Rust と Cargo が必要です。

Rust が未導入の場合は、公式サイトの手順に従って Rust toolchain をインストールします。

```bash
rustc --version
cargo --version
```

上のコマンドでバージョンが表示されれば準備完了です。

このリポジトリを取得して、プロジェクトディレクトリでビルドします。

```bash
cargo build
```

開発中のバイナリをそのまま実行する場合は、次のように `cargo run` を使えます。

```bash
cargo run -- examples/basic.farmrs
```

`farmrs` コマンドとして使いたい場合は、ローカル環境にインストールします。

```bash
cargo install --path .
```

インストール後、次のコマンドでヘルプが表示されれば利用できます。

```bash
farmrs --help
```

## リリース用実行ファイルの作り方

Windows 向けの実行ファイルは、ローカルでreleaseビルドしてGitHub Releaseへ添付します。

```bash
cargo build --release
```

GitHub上でReleaseを作る場合は、`v0.1.0` のような `v` で始まるタグを使います。

```bash
git tag v0.1.0
git push origin v0.1.0
```

Windows 向けには、Release の Downloads に `farmrs-windows-x86_64.exe` を直接添付します。zipを展開しなくても、そのファイルだけをダウンロードして実行できます。

## 使い方

標準出力に変換結果を表示する場合:

```bash
farmrs my_script.farmrs
```

`.py` ファイルとして出力する場合:

```bash
farmrs my_script.farmrs -o my_script.py
```

`cargo run` 経由で実行する場合は、`--` の後に `farmrs` へ渡す引数を書きます。

```bash
cargo run -- examples/basic.farmrs -o output.py
```

## 例

入力:

```rust
fn main() {
    loop {
        if can_harvest() {
            harvest();
        } else {
            move_dir(Direction::East);
        }
    }
}
```

出力:

```python
while True:
    if can_harvest():
        harvest()
    else:
        move(East)
```

## MVP で対応している構文

- `fn main`
- 補助関数としての `fn`
- `loop`
- `while`
- `if`、`else if`、`else`
- `for i in a..b`
- `let`、`let mut`
- 代入
- 関数呼び出しと式文
- `break`、`continue`、`return`
- `//` コメント

## 式の変換ルール

- `true` と `false` は `True` と `False` に変換されます。
- `&&`、`||`、`!` は `and`、`or`、`not` に変換されます。
- `A::B` は `A.B` に変換されます。
- `Entity::X`、`Ground::X`、`Item::X` は `Entities.X`、`Grounds.X`、`Items.X` に変換されます。
- `Direction::North`、`Direction::East`、`Direction::South`、`Direction::West` は、名前空間なしの方向名に変換されます。
- `move_dir(...)` は `move(...)` に変換されます。

## 対応していない構文

- Rust の所有権
- 借用
- ライフタイム
- trait
- `impl`
- generics
- macro
- module
- `use` 文
