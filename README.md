# farmrs

`farmrs` は [The Farmer Was Replaced](https://thefarmerwasreplaced.wiki.gg/) 向けの Rust 風 DSL トランスパイラです。
`.rs` または `.farmrs` ファイルを書き、ゲーム内で実行できる Python 風コードへ変換します。
普段はエディタの Rust ハイライトが効きやすい `.rs` を `rs_src/` に置く運用がおすすめです。

これは Rust コンパイラではありません。所有権、借用、ライフタイム、trait bound、macro 展開、crate 解決、Rust 標準ライブラリの実行時挙動は再現しません。ただし、IDE 補助やコード整理に使う Rust 風の宣言は、できるだけゲーム用 Python 風コードへ安全に落とすか、出力なしのメタデータとして受理します。

## 導入

`farmrs` を使うには、配布済みの実行ファイル、または Rust/Cargo のどちらかが必要です。

配布済みの実行ファイルを使う場合:

```powershell
.\farmrs.exe --help
.\farmrs.exe my_script.rs -o my_script.py
```

macOS / Linux で配布ファイルを直接使う場合:

```bash
chmod +x farmrs
./farmrs --help
./farmrs my_script.rs -o my_script.py
```

ソースから使う場合は Rust toolchain を入れてから確認します。

```bash
rustc --version
cargo --version
cargo build
```

開発中のバイナリをそのまま実行する場合:

```bash
cargo run -- examples/basic.farmrs -o output.py
```

ローカルの `farmrs` コマンドとして入れる場合:

```bash
cargo install --path .
farmrs --help
```

リリース用の実行ファイルを作る場合:

```bash
cargo build --release
```

## クイックスタート

1. `.rs` ファイルを作ります。

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

2. `.py` に変換します。

```bash
cargo run -- my_script.rs -o my_script.py
```

構文だけ確認する場合:

```bash
cargo run -- my_script.rs --check
```

配布済みの Windows 実行ファイルを使う場合:

```powershell
.\farmrs.exe my_script.rs -o my_script.py
.\farmrs.exe my_script.rs --check
```

3. 生成された `.py` の中身をゲーム側のエディタ、または外部エディタ連携先のファイルで使います。

出力例:

```python
while True:
    if can_harvest():
        harvest()
    else:
        move(East)
```

標準出力に変換結果を表示するだけなら `-o` を省略できます。

```bash
cargo run -- examples/basic.farmrs
```

もう少し大きい例として、列を回りながら畑を手入れする `examples/crop_columns.farmrs` も入っています。

```bash
cargo run -- examples/crop_columns.farmrs --check
cargo run -- examples/crop_columns.farmrs -o crop_columns.py
```

## rs_src / py_src 運用

実プレイ用には、Rust 風コードを書くフォルダと、ゲームへ渡す Python 風コードの出力フォルダを分けると楽です。

```text
rs_src/
  Cargo.toml
  main.rs
  crops/carrot.rs
py_src/
  main.py
  crops/carrot.py
```

既定では `rs_src` の `.rs` / `.farmrs` を探し、同じ相対パスで `py_src` に `.py` を出力します。

```bash
cargo run -- --sync
```

保存するたびに自動変換したい場合は、監視モードを起動します。起動時に一度まとめて変換し、その後は `.rs` / `.farmrs` の新規作成や更新を検出して変換します。

```bash
cargo run -- --watch
```

フォルダ名を変えたい場合:

```bash
cargo run -- --sync --src my_rs --out game_py
cargo run -- --watch --src my_rs --out game_py
```

削除された `.rs` / `.farmrs` に対応する `.py` は自動削除しません。ゲーム側で使っているファイルを誤って消さないため、不要になった `.py` は手動で整理してください。

## IDE 補助

`rs_src/*.rs` は Rust ファイルとして編集できるため、Cursor や rust-analyzer の補完を使いやすくなります。ただし `harvest()` や `Entity::Carrot` はゲーム独自 API なので、そのままだと Rust 側では未定義になります。

そのため、IDE 用の緩衝ライブラリとして `farmrs::prelude` を用意しています。これはゲーム動作を再現するものではなく、未定義エラーを減らして Rust 構文を書きやすくするための空実装です。

まず IDE 用 Cargo 設定を作ります。

```bash
cargo run -- --init-ide
```

`rs_src/main.rs` の例:

```rust
use farmrs::prelude::*;

fn should_harvest(entity: Entity) -> bool {
    if entity == Entity::Carrot {
        return can_harvest();
    }
    return false;
}

fn main() {
    if should_harvest(Entity::Carrot) {
        harvest();
        trade_n(Item::Carrot_Seed, 10);
        use_item_n(Item::Fertilizer, 2);
        measure_dir(Direction::North);
    }
}
```

Rust として確認する場合:

```bash
cargo check --manifest-path rs_src/Cargo.toml
```

ゲーム用 `.py` に変換する場合:

```bash
cargo run -- --sync
```

`use farmrs::prelude::*;` はトランスパイル時に無視され、出力には残りません。Rust は関数オーバーロードができないため、複数引数のゲーム API には Rust 風 alias を使います。

| IDE 用 | 出力 |
| --- | --- |
| `trade_n(Item::Carrot_Seed, 10);` | `trade(Items.Carrot_Seed, 10)` |
| `use_item_n(Item::Fertilizer, 2);` | `use_item(Items.Fertilizer, 2)` |
| `measure_dir(Direction::North);` | `measure(North)` |

## farmrs 構文

対応している主な構文:

- `fn main() { ... }`
- 補助関数としての `fn helper(...) { ... }`
- 関数引数・戻り値の型注釈
- 関数 generics、例: `fn helper<T>(...)`
- `struct` 宣言と struct literal
- `enum` 宣言と custom enum variant
- `mod` 内の関数
- `impl` 内の関数
- `trait` 宣言
- `macro_rules!` / `macro` 宣言
- `loop`
- `while`
- `if`、`else if`、`else`
- `for i in a..b`
- `for item in collection`
- `let`、`let mut`
- 代入
- list / dict / set / tuple リテラル
- 添字アクセス、例: `xs[i]`
- 関数呼び出しと式文
- デフォルト引数つき関数定義
- ネストした関数定義
- メソッド呼び出し風の式文、例: `xs.append(1);`
- `break`、`continue`、`return`
- `//` コメント
- `/* ... */` ブロックコメント

主な変換ルール:

| farmrs | 出力 |
| --- | --- |
| `true` / `false` | `True` / `False` |
| `&&` / `||` / `!` | `and` / `or` / `not` |
| `for i in 0..10 { ... }` | `for i in range(10):` |
| `for i in 2..10 { ... }` | `for i in range(2, 10):` |
| `for item in xs { ... }` | `for item in xs:` |
| `[1, 2]` | `[1, 2]` |
| `{Item::Carrot_Seed: 10}` | `{Items.Carrot_Seed: 10}` |
| `(1, 2)` | `(1, 2)` |
| `xs[i]` | `xs[i]` |
| `fn step(n: i32 = 1) { ... }` | `def step(n=1):` |
| `fn outer() { fn inner() { ... } }` | ネストした `def inner():` |
| `struct Plan { count: i32 }` | `def Plan(count=None): return {"count": count}` |
| `Plan { count: 10 }` | `Plan(count=10)` |
| `enum Crop { Carrot }` | `Crop_Carrot = "Crop.Carrot"` |
| `Crop::Carrot` | `Crop_Carrot` |
| `mod helpers { fn clear() { ... } }` | `def helpers_clear():` |
| `helpers::clear()` | `helpers_clear()` |
| `impl Plan { fn make() { ... } }` | `def Plan_make():` |
| `Plan::make()` | `Plan_make()` |
| `trait Tool { ... }` | 出力なしのメタデータとして受理 |
| `macro_rules! name { ... }` | 出力なしのメタデータとして受理 |
| `a // b` / `a ** b` | `a // b` / `a ** b` |
| `move_dir(Direction::East)` | `move(East)` |
| `Direction::North` / `East` / `South` / `West` | `North` / `East` / `South` / `West` |
| `Entity::Bush` | `Entities.Bush` |
| `Ground::Soil` | `Grounds.Soil` |
| `Item::Fertilizer` | `Items.Fertilizer` |
| `Unlock::Carrots` | `Unlocks.Carrots` |
| `Leaderboard::Fastest_Reset` | `Leaderboards.Fastest_Reset` |
| 未登録の `A::B` | `A.B` |

`fn main()` はゲーム側のトップレベルコードとして出力されます。補助関数は `def helper(...):` として出力されます。

`.rs` として rust-analyzer の型チェックまで通したい場合は、Rust として正しい構文を使ってください。デフォルト引数、dict / set リテラル、`//` 除算、`**` べき乗はゲーム側 Python 風コードに寄せた `farmrs` 構文なので、必要なら `.farmrs` ファイルで使うのが安全です。

構文エラーや未対応構文は、日本語でファイル名・行・列・理由を表示します。

```text
エラー: examples/bad.farmrs:3行1列: 式文の後に `;` が必要です
```

## ゲーム API 対応表

関数呼び出しは基本的にそのまま出力します。`farmrs` が行うのは、Rust 風の名前空間や真偽値、演算子、範囲ループなどの表記変換です。ゲーム API が現在アンロック済みかどうか、ゲーム内で成功するかどうかは検証しません。

公式情報は、[Steam の開発者投稿](https://steamcommunity.com/app/2060160/discussions/0/3806155895394113592/) で案内されている [公式 Wiki](https://thefarmerwasreplaced.wiki.gg/) を基準にしています。関数一覧は [Available Functions](https://thefarmerwasreplaced.wiki.gg/wiki/Available_Functions)、補完情報は [Tooltips Code](https://thefarmerwasreplaced.wiki.gg/wiki/Tooltips_Code)、Leaderboard 関連は [Leaderboard](https://thefarmerwasreplaced.wiki.gg/wiki/Leaderboard) を参照してください。

| 公式API | farmrsでの書き方 | 出力 | メモ |
| --- | --- | --- | --- |
| `harvest()` | `harvest();` | `harvest()` | そのまま |
| `can_harvest()` | `can_harvest()` | `can_harvest()` | 条件式で利用可能 |
| `swap(direction)` | `swap(Direction::North);` | `swap(North)` | 方向は名前空間なしに変換 |
| `range()` | `for i in 0..10 { ... }` | `for i in range(10):` | ループは `a..b` 推奨。式中の `range(10)` はそのまま |
| `plant(entity)` | `plant(Entity::Bush);` | `plant(Entities.Bush)` | `Entities::Bush` も `Entities.Bush` へ変換 |
| `move(direction)` | `move_dir(Direction::East);` | `move(East)` | `move(Direction::East);` も利用可能 |
| `till()` | `till();` | `till()` | そのまま |
| `trade(item)` | `trade(Item::Carrot_Seed);` | `trade(Items.Carrot_Seed)` | 複数購入は `trade(Item::Carrot_Seed, 10);` |
| `get_pos_x()` | `get_pos_x()` | `get_pos_x()` | そのまま |
| `get_pos_y()` | `get_pos_y()` | `get_pos_y()` | そのまま |
| `get_world_size()` | `get_world_size()` | `get_world_size()` | そのまま |
| `get_entity_type()` | `get_entity_type()` | `get_entity_type()` | `Entity::X` との比較に使える |
| `get_ground_type()` | `get_ground_type()` | `get_ground_type()` | `Ground::Soil` との比較に使える |
| `get_tick_count()` | `get_tick_count()` | `get_tick_count()` | 実行 tick 計測 |
| `get_time()` | `get_time()` | `get_time()` | そのまま |
| `get_op_count()` | `get_op_count()` | `get_op_count()` | 公式では削除済み扱い。`get_tick_count()` 推奨 |
| `use_item(item, n=1)` | `use_item(Item::Fertilizer);` | `use_item(Items.Fertilizer)` | 回数指定もそのまま |
| `get_water()` | `get_water()` | `get_water()` | そのまま |
| `do_a_flip()` | `do_a_flip();` | `do_a_flip()` | そのまま |
| `print(something)` | `print("ground:", get_ground_type());` | `print("ground:", get_ground_type())` | 複数引数もそのまま |
| `quick_print()` | `quick_print("hi");` | `quick_print("hi")` | そのまま |
| `len(collection)` | `len(xs)` | `len(xs)` | list / dict / tuple リテラルや添字アクセスと併用可能 |
| `num_items(item)` | `num_items(Item::Fertilizer)` | `num_items(Items.Fertilizer)` | そのまま |
| `get_cost(thing)` | `get_cost(Unlock::Carrots)` | `get_cost(Unlocks.Carrots)` | `Item::X` / `Entity::X` / `Unlock::X` を利用可能 |
| `clear()` | `clear();` | `clear()` | そのまま |
| `get_companion()` | `get_companion()` | `get_companion()` | 戻り値は変数に入れて添字アクセスできる |
| `unlock(unlock)` | `unlock(Unlock::Carrots);` | `unlock(Unlocks.Carrots)` | そのまま |
| `num_unlocked(thing)` | `num_unlocked(Unlock::Multi_Trade)` | `num_unlocked(Unlocks.Multi_Trade)` | そのまま |
| `timed_reset()` | `timed_reset();` | `timed_reset()` | そのまま |
| `measure()` | `measure();` | `measure()` | `measure(Direction::North);` は `measure(North)` |
| `min(a,b)` | `min(a, b)` | `min(a, b)` | そのまま |
| `max(a,b)` | `max(a, b)` | `max(a, b)` | そのまま |
| `abs(number)` | `abs(-1)` | `abs( - 1)` | Python として有効 |
| `random()` | `random()` | `random()` | そのまま |
| `list()` | `let xs = list();` / `let xs = [1, 2];` | `xs = list()` / `xs = [1, 2]` | 空リストは `list()`、値入りはリテラルも利用可能 |
| `set()` | `let seen = set();` / `let seen = {1, 2};` | `seen = set()` / `seen = {1, 2}` | 空セットは `set()`、値入りはリテラルも利用可能 |
| `dict()` | `let costs = dict();` / `let costs = {Item::Carrot_Seed: 10};` | `costs = dict()` / `costs = {Items.Carrot_Seed: 10}` | 辞書リテラルと添字アクセスに対応 |
| `set_execution_speed(speed)` | `set_execution_speed(1);` | `set_execution_speed(1)` | そのまま |
| `set_farm_size(size)` | `set_farm_size(5);` | `set_farm_size(5)` | そのまま |
| `simulate(...)` | `simulate("main.py", [Unlock::Carrots], {Item::Carrot_Seed: 10}, {"x": 1}, 0, 1);` | `simulate("main.py", [Unlocks.Carrots], {Items.Carrot_Seed: 10}, {"x": 1}, 0, 1)` | list / dict リテラルを含む公式例も書ける |
| `leaderboard_run(...)` | `leaderboard_run(Leaderboard::Fastest_Reset, filename, speedup);` | `leaderboard_run(Leaderboards.Fastest_Reset, filename, speedup)` | Leaderboard ページの関連 API |

## ゲーム内機能と farmrs の対応範囲

ゲーム内の Python 風コードでよく使う要素は、できるだけ Rust 風ファイルからそのまま出力できるようにしています。ゲーム API の関数呼び出しは多くがそのまま通りますが、実行時のゲーム状態だけはゲーム本体の中でしか確定しません。

| ゲーム内では使える要素 | farmrs の現状 | メモ |
| --- | --- | --- |
| list / dict / tuple リテラル | 対応 | `[1, 2]`、`{Item::Carrot_Seed: 10}`、`(1, 2)` |
| 添字アクセス | 対応 | `xs[i]`、`costs[Item::Carrot_Seed]` の読み書き |
| collection への `for` | 対応 | `for item in xs { ... }` |
| デフォルト引数つき関数定義 | 対応 | `fn step(n: i32 = 1) { ... }` は `def step(n=1):` |
| ネストした関数定義 | 対応 | ブロック内の `fn` は Python のネスト `def` になる |
| ブロックコメント | 対応 | `/* ... */` は `# ...` へ変換 |
| Python 風の `//` / `**` 演算子 | 対応 | `//` は文末コメントと衝突しない範囲で除算演算子として扱う |
| `simulate()` 用の複雑な dict/globals 指定 | 対応 | list / dict リテラルを使って公式例に近い形で書ける |
| ゲーム内の unlock 状態・所持数・実行結果 | ゲーム内で確定 | 変換器は静的なコード生成だけを行う |
| ゲーム API の実際の挙動 | ゲーム内で実行 | `farmrs::prelude` は IDE 補完用の空実装 |

迷った場合は、まず `.rs` で書いて `cargo run -- --sync` または `cargo run -- file.rs --check` を実行してください。構文エラーはファイル名・行・列つきの日本語エラーになります。

## 制限事項

`farmrs` はゲーム用 Python 風コードを生成するための小さなトランスパイラで、Rust そのものを実装するものではありません。Rust 風の宣言は受理しますが、ゲーム側で意味を持つ形に単純化します。

- `struct` は辞書を返すファクトリ関数になります。フィールド参照は `plan["count"]` のように添字アクセスで書いてください。
- `enum` は文字列定数になります。variant に値を持たせる Rust enum は、variant 名だけを定数化します。
- `mod` と `impl` 内の関数は、`helpers::clear()` -> `helpers_clear()`、`Plan::make()` -> `Plan_make()` のように平坦化します。
- `trait` 宣言と `macro_rules!` / `macro` 宣言は、IDE 補助用のメタデータとして受理し、ゲーム用出力には残しません。macro の展開は行いません。

Rust の所有権・借用・ライフタイムの意味論は再現しません。ただし、関数引数や戻り値の型注釈に出てくる `&` や lifetime は読み飛ばせます。トップレベルやブロック内の `use ...;` は、IDE 補助用として読み飛ばします。`fn helper<T>(...)` のような関数 generics も、出力では削除します。

ゲーム内の Python 風コードを完全に実行・検証するツールではありません。`farmrs` は、Rust 風に書いた自動化スクリプトをゲーム用コードへ変換するためのツールです。

## 検証方法

Rust/Cargo が使える環境で次を実行します。

```bash
cargo build
cargo fmt --check
cargo test
cargo run -- --help
cargo run -- examples/basic.farmrs
cargo run -- examples/basic.farmrs -o output.py
cargo run -- examples/basic.farmrs --check
cargo run -- examples/crop_columns.farmrs --check
cargo run -- --sync
cargo run -- --init-ide
cargo check --manifest-path rs_src/Cargo.toml
```

期待される `examples/basic.farmrs` の出力:

```python
while True:
    if can_harvest():
        harvest()
    else:
        move(East)
```

`cargo` が見つからない場合は、Rust toolchain をインストールするか、配布済みの `farmrs` 実行ファイルを使ってください。
