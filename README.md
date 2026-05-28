# farmrs

`farmrs` は [The Farmer Was Replaced](https://thefarmerwasreplaced.wiki.gg/) 向けの Rust 風 DSL トランスパイラです。
`.rs` または `.farmrs` ファイルを書き、ゲーム内で実行できる Python 風コードへ変換します。
普段はエディタの Rust ハイライトが効きやすい `.rs` を `rs_src/` に置く運用がおすすめです。

これは Rust コンパイラではありません。所有権、借用、ライフタイム、trait bound、macro 展開、crate 解決、Rust 標準ライブラリの実行時挙動は再現しません。ただし、IDE 補助やコード整理に使う Rust 風の宣言は、できるだけゲーム用 Python 風コードへ安全に落とすか、出力なしのメタデータとして受理します。

## Windows 版の使い方

`farmrs` は Windows の `farmrs.exe` をダブルクリックして使う運用を基本にしています。このREADMEの導入手順もWindows専用です。Rust 風コードは `rs_src` に書き、ゲームが読む `.py` は Save フォルダへ自動出力します。

```text
好きな作業フォルダ/
  farmrs.exe
  farmrs.toml        パス設定。GUIで自動作成
  rs_src/
    Cargo.toml       IDE補助用。自動作成
    .farmrs_ide/     IDE補助用。自動作成
    main.rs          ここにRust風コードを書く

ゲームのSaveフォルダ/
  __builtins__.py    ゲーム側が作る補完用ファイル
  main.py            farmrsが出力するゲーム用コード
```

`rs_src` は自分が編集する場所です。ゲームはここを直接読みません。ゲームが読むのは Save フォルダ内の `main.py` です。

## 初回セットアップ

1. 好きな場所に作業フォルダを作ります。例: `C:\Users\YourName\Desktop\farming`
2. その中に `farmrs.exe` を置きます。
3. `farmrs.exe` をダブルクリックします。
4. 初回起動時に `farmrs.toml`、`rs_src/main.rs`、IDE補助用ファイルが自動作成されます。
5. ウィンドウの `ゲームの Save フォルダ` に、ゲーム側のセーブフォルダを指定します。
6. `保存` を押します。

初回生成直後は `out_dir` が空欄なので、Saveフォルダを選ぶまでは監視を開始しません。Saveフォルダを指定すると自動変換が有効になります。`main.rs` を保存すると、Save フォルダの `main.py` が更新されます。`main.py` がゲーム側などで空に戻された場合も、対応する `.rs` があれば再生成します。

## Save フォルダの見つけ方

[公式 Wiki の External Editor](https://thefarmerwasreplaced.wiki.gg/wiki/External_editor) では、外部エディタ用の Save フォルダはゲーム内の `Load` メニューにある `Open Folder` ボタンから開ける、と説明されています。

1. The Farmer Was Replaced を起動します。
2. `Load` メニューを開きます。
3. 使いたいセーブを選びます。
4. `Open Folder` を押します。
5. 開いたフォルダのパスを、`farmrs.exe` のウィンドウに指定します。

開いた場所に `__builtins__.py` や既存の `.py` が見えていれば、そのフォルダが指定先です。`Saves` という親フォルダではなく、実際に `.py` が入っているセーブ個別のフォルダを指定してください。

## main.rs の最小例

```rust
use farmrs::prelude::*;

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

生成される `main.py`:

```python
while True:
    if can_harvest():
        harvest()
    else:
        move(East)
```

`use farmrs::prelude::*;` はCursorなどのIDEで未定義エラーを減らすための行です。変換後の `.py` には出力されません。

## farmrs.exe の入手

GitHubで配布されている場合:

1. ブラウザで [Aeniminu/farmrs](https://github.com/Aeniminu/farmrs) を開きます。
2. `Releases` を押します。
3. 一番上の `Latest` リリースを開きます。
4. `Assets` を開きます。
5. Windows 用の `.exe` をダウンロードします。
6. 実プレイ用の作業フォルダに置き、必要なら名前を `farmrs.exe` に変更します。

`Releases` や `.exe` が無い場合は、ソースから自分で作ります。このリポジトリの `Cargo.toml` がある場所で次を実行します。

```powershell
cargo build --release
```

作成された `target\release\farmrs.exe` を、実プレイ用の作業フォルダへ置きます。

## farmrs.toml の役割

`farmrs.toml` は `farmrs.exe` の設定メモです。GUIで選んだ2つのパスが保存されます。

```toml
src_dir = "C:\\Users\\YourName\\Desktop\\farming\\rs_src"
out_dir = "C:\\Users\\YourName\\AppData\\LocalLow\\TheFarmerWasReplaced\\TheFarmerWasReplaced\\Saves\\Save0"
```

ゲームでいうと「このセーブではこの設定を使う」という保存データに近いです。Cargo の `Cargo.toml` はRustプロジェクトの設計図、`farmrs.toml` は変換器の行き先メモ、と分けて考えると分かりやすいです。

## IDE 補助

`rs_src/*.rs` はRustファイルなので、Cursorやrust-analyzerの補完を使いやすくなります。ただし `harvest()` や `Entity::Carrot` はゲーム独自APIなので、そのままだとRust側では未定義になります。

そのため、`farmrs.exe` は `rs_src/Cargo.toml` と `rs_src/.farmrs_ide/` を自動生成します。これはゲーム動作を再現するものではなく、未定義エラーを減らしてRust構文を書きやすくするための空実装です。

Rust として確認する場合は、PowerShellで `rs_src` のある作業フォルダから次を実行します。

```powershell
cargo check --manifest-path rs_src\Cargo.toml
```

Rust は関数オーバーロードができないため、複数引数のゲームAPIにはRust風aliasを使います。

| IDE 用 | 出力 |
| --- | --- |
| `trade_n(Item::Carrot_Seed, 10);` | `trade(Items.Carrot_Seed, 10)` |
| `use_item_n(Item::Fertilizer, 2);` | `use_item(Items.Fertilizer, 2)` |
| `measure_dir(Direction::North);` | `measure(North)` |

## コマンドで使う場合

通常はダブルクリックで十分です。問題の切り分けをしたいときだけ、PowerShellで `farmrs.exe` のある作業フォルダへ移動して使います。

```powershell
.\farmrs.exe --help
.\farmrs.exe --sync --src rs_src --out "ゲームのSaveフォルダ"
.\farmrs.exe --watch --src rs_src --out "ゲームのSaveフォルダ"
.\farmrs.exe rs_src\main.rs --check
```

`--out` を省略すると、既定では `py_src` に出力します。実プレイでは基本的にSaveフォルダを指定してください。削除された `.rs` に対応する `.py` は自動削除しません。ゲーム側で使っているファイルを誤って消さないため、不要な `.py` は手動で整理してください。

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

迷った場合は、まず `.rs` で書いてGUIを起動するか、PowerShellで `.\farmrs.exe --sync --src rs_src --out "ゲームのSaveフォルダ"` または `.\farmrs.exe rs_src\main.rs --check` を実行してください。構文エラーはファイル名・行・列つきの日本語エラーになります。

## 制限事項

`farmrs` はゲーム用 Python 風コードを生成するための小さなトランスパイラで、Rust そのものを実装するものではありません。Rust 風の宣言は受理しますが、ゲーム側で意味を持つ形に単純化します。

- `struct` は辞書を返すファクトリ関数になります。フィールド参照は `plan["count"]` のように添字アクセスで書いてください。
- `enum` は文字列定数になります。variant に値を持たせる Rust enum は、variant 名だけを定数化します。
- `mod` と `impl` 内の関数は、`helpers::clear()` -> `helpers_clear()`、`Plan::make()` -> `Plan_make()` のように平坦化します。
- `trait` 宣言と `macro_rules!` / `macro` 宣言は、IDE 補助用のメタデータとして受理し、ゲーム用出力には残しません。macro の展開は行いません。

Rust の所有権・借用・ライフタイムの意味論は再現しません。ただし、関数引数や戻り値の型注釈に出てくる `&` や lifetime は読み飛ばせます。トップレベルやブロック内の `use ...;` は、IDE 補助用として読み飛ばします。`fn helper<T>(...)` のような関数 generics も、出力では削除します。

ゲーム内の Python 風コードを完全に実行・検証するツールではありません。`farmrs` は、Rust 風に書いた自動化スクリプトをゲーム用コードへ変換するためのツールです。

## 検証方法

開発者がソースから確認する場合は、Windows PowerShellでこのリポジトリの `Cargo.toml` がある場所から実行します。

```powershell
cargo fmt --check
cargo build
cargo test
cargo build --release
.\target\release\farmrs.exe --help
.\target\release\farmrs.exe examples\basic.farmrs
.\target\release\farmrs.exe examples\basic.farmrs -o output.py
.\target\release\farmrs.exe examples\basic.farmrs --check
.\target\release\farmrs.exe examples\crop_columns.farmrs --check
.\target\release\farmrs.exe --sync
.\target\release\farmrs.exe --init-ide
cargo check --manifest-path rs_src\Cargo.toml
```

期待される `examples/basic.farmrs` の出力:

```python
while True:
    if can_harvest():
        harvest()
    else:
        move(East)
```

`cargo` が見つからない場合は、Rust toolchain をインストールするか、配布済みの `farmrs.exe` を使ってください。実プレイでは、上の検証コマンドよりも `farmrs.exe` のダブルクリックGUIを使うのが簡単です。
