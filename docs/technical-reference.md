# Transplanter 技術メモ

この文書は、READMEから外した仕様・検証・開発者向け情報をまとめたものです。初回導入だけ知りたい場合は、先に [README.md](../README.md) を読んでください。

## transplanter.toml の役割

`transplanter.toml` はTransplanterの設定メモです。GUIで選んだパスと言語モードが保存されています。

```toml
src_dir = "C:\\Users\\YourName\\Desktop\\farming\\play_src"
out_dir = "C:\\Users\\YourName\\AppData\\LocalLow\\TheFarmerWasReplaced\\TheFarmerWasReplaced\\Saves\\Save0"
language = "rust"
```

ゲームでいうと「このセーブではこの畑とこの納品先を使う」という保存データに近いです。Cargo の `Cargo.toml` はRustプロジェクトの設計図、`transplanter.toml` は変換器の行き先メモ、と分けて考えると分かりやすいです。

`language` は `rust` / `lisp` / `auto` のどれかです。新規GUIワークスペースは `rust` から始まります。古い設定ファイルに `language` がない場合は、互換性のため `auto` として読みます。

新規作成時の既定ソースフォルダは `play_src` です。古い環境で `src_dir` が実在する `rs_src` を指している場合は、その設定をそのまま尊重します。`rs_src` が見つからない場合は、新しい既定の `play_src` に最初のファイルを作ります。

## IDE 補助

`play_src/*.rs` はRustファイルなので、Cursorやrust-analyzerの補完を使えます。ただし `harvest()` や `Entity::Carrot` はゲーム独自APIなので、そのままだとRust側では未定義になります。

そのため、Transplanter は作業フォルダ直下に `Cargo.toml` と `.transplanter_ide/transplanter_rust/` を自動生成します。`play_src` の中にはユーザーが読む・書く `.rs` / `.scm` / `.lisp` だけを置く方針です。実際に変換する対象は言語モードで絞られます。

Rustとして確認する場合は、PowerShellで `Transplanter.exe` のある作業フォルダから次を実行します。

```powershell
cargo check --manifest-path Cargo.toml
```

Rust は関数オーバーロードができないため、複数引数のゲームAPIにはRust風aliasを使います。

| IDE 用 | 出力 |
| --- | --- |
| `trade_n(Item::Carrot_Seed, 10);` | `trade(Items.Carrot_Seed, 10)` |
| `use_item_n(Item::Fertilizer, 2);` | `use_item(Items.Fertilizer, 2)` |
| `measure_dir(Direction::North);` | `measure(North)` |

## コマンドで使う場合

通常はダブルクリックで十分です。問題の切り分けをしたいときだけ、PowerShellで `Transplanter.exe` のある作業フォルダへ移動して使います。

```powershell
.\Transplanter.exe --help
.\Transplanter.exe --sync --src play_src --out "ゲームのSaveフォルダ"
.\Transplanter.exe --sync --src play_src --out "ゲームのSaveフォルダ" --language rust
.\Transplanter.exe --sync --src play_src --out "ゲームのSaveフォルダ" --language lisp
.\Transplanter.exe --watch --src play_src --out "ゲームのSaveフォルダ"
.\Transplanter.exe play_src\main.rs --check
.\Transplanter.exe play_src\main.scm --check
```

`--language` は `--sync` / `--watch` で使います。`rust` は `.rs` だけ、`lisp` は `.scm` / `.lisp` だけ、`auto` は現在のフォルダ内の対応拡張子を自動判定します。単体ファイル変換は入力パスが明示されているため、今まで通り拡張子で判定します。

`--out` を省略すると、既定では `py_src` に出力します。実プレイでは基本的にSaveフォルダを指定してください。削除されたソースや、選択中の言語モードから外れたソースに対応する `.py` は自動削除しません。ゲーム側で使っているファイルを誤って消さないため、不要な `.py` は手動で整理してください。

## Rust 入力の書き方

対応している主な構文:

- `fn main() { ... }`
- 補助関数としての `fn helper(...) { ... }`
- 関数引数・戻り値の型注釈
- `loop`、`while`
- `if`、`else if`、`else`
- `for i in a..b`
- `for item in collection`
- `let`、`let mut`、`let name: Type = value`
- 代入、添字アクセス、関数呼び出し
- list / tuple リテラル
- `struct`、`enum`、`mod`、`impl`、`trait`
- `break`、`continue`、`return`
- `//` コメント、`/* ... */` ブロックコメント

入力ではRustとして成立する表記だけを受け付けます。出力後のPython風表記を `.rs` に書いた場合はエラーになります。

主な変換ルール:

| Rust入力 | 出力 |
| --- | --- |
| `true` / `false` | `True` / `False` |
| `&&` / `||` / `!` | `and` / `or` / `not` |
| `for i in 0..10 { ... }` | `for i in range(10):` |
| `for i in 2..10 { ... }` | `for i in range(2, 10):` |
| `for item in xs { ... }` | `for item in xs:` |
| `move_dir(Direction::East)` | `move(East)` |
| `Direction::North` | `North` |
| `Entity::Bush` | `Entities.Bush` |
| `Ground::Soil` | `Grounds.Soil` |
| `Item::Fertilizer` | `Items.Fertilizer` |
| `Unlock::Carrots` | `Unlocks.Carrots` |
| `Leaderboard::Fastest_Reset` | `Leaderboards.Fastest_Reset` |

次のようなPython風・出力後形式は入力では使えません。

| NG | OK |
| --- | --- |
| `while True { ... }` | `loop { ... }` または `while true { ... }` |
| `for i in range(4) { ... }` | `for i in 0..4 { ... }` |
| `plant(Entities.Bush);` | `plant(Entity::Bush);` |
| `move(North);` | `move_dir(Direction::North);` |
| `get_ground_type() != Grounds.Soil` | `get_ground_type() != Ground::Soil` |
| `and` / `or` / `not` | `&&` / `||` / `!` |
| `{Item::Carrot_Seed: 10}` | `let mut costs = dict(); costs[Item::Carrot_Seed] = 10;` |
| `fn step(n: i32 = 1) { ... }` | `fn step(n: i32) { ... }` |
| `a // b` / `a ** b` | Rustとして成立する式 |

## Scheme風Lisp 入力の書き方

`.scm` / `.lisp` を変換するには、Transplanter本体に加えて Guile Scheme または Chez Scheme が必要です。どれか1つがPATHから起動できれば使えます。

```powershell
guild --version
chezscheme --version
scheme --version
```

検査はこの順番で試します。

1. `guild compile`、Guile Scheme
2. `chezscheme --script`、Chez Scheme
3. `scheme --script`、Chez Scheme系コマンド

対応している主な構文:

- `(define (main) ...)`
- `(define (helper x) ...)`
- `(loop ...)`
- `(while condition ...)`
- `(if condition then [else])`
- `(begin ...)`
- `(for i start end ...)`
- `(let ((name value) ...) ...)`
- `(set! name value)`
- `(index xs i)` と `(set-index! xs i value)`
- `#t` / `#f`
- `+`、`-`、`*`、`/`、`=`、`!=`、`<`、`<=`、`>`、`>=`、`and`、`or`、`not`
- `;` 行コメント

主な変換ルール:

| Lisp入力 | 出力 |
| --- | --- |
| `(define (main) (harvest))` | `harvest()` |
| `(define (water) ...)` | `def water():` |
| `(loop ...)` | `while True:` |
| `(for i 0 6 ...)` | `for i in range(0, 6):` |
| `(do-a-flip)` | `do_a_flip()` |
| `(move :east)` | `move(East)` |
| `(plant (entity bush))` | `plant(Entities.Bush)` |
| `(if (!= (get-ground-type) (ground soil)) (till))` | `if (get_ground_type() != Grounds.Soil):` |
| `(use-item (item fertilizer))` | `use_item(Items.Fertilizer)` |

Lisp版はSchemeそのものを丸ごと実装する処理系ではありません。macro、quote、末尾再帰最適化、ライブラリ読み込み、本物のレキシカルスコープの完全再現はまだ対象外です。ただし、`.py` を出力する前に、Transplanter用の検査preludeを付けた状態で Guile Scheme または Chez Scheme に渡します。

## ファイルを分ける場合

`play_src` の中では、Rustの `mod` と同じ感覚で同じフォルダの別ファイルを参照できます。

```text
play_src/
  main.rs
  farmlab.rs
```

`main.rs`:

```rust
use transplanter_rust::prelude::*;

mod farmlab;

fn main() {
    farmlab::main();
}
```

`farmlab.rs`:

```rust
use transplanter_rust::prelude::*;

pub fn main() {
    print("test_text");
}
```

出力は `main.py` と `farmlab.py` になります。`main.py` には `import farmlab` が入り、`farmlab::main();` は `farmlab.main()` になります。`farmlab.rs` 側の `pub fn main()` は自動実行されず、Pythonの `def main():` として出力されます。

今の対応範囲は、同じフォルダにある `mod farmlab;` -> `farmlab.rs` です。`mod.rs` 形式やサブフォルダ module は今後の拡張候補です。

## ゲーム API 対応表

関数呼び出しは基本的にそのまま出力します。Transplanter が行うのは、RustやLisp側の名前、真偽値、演算子、範囲ループなどをゲーム用Pythonへ変換することです。ゲーム API が現在アンロック済みかどうか、ゲーム内で成功するかどうかは検証しません。

公式情報は、[Steam の開発者投稿](https://steamcommunity.com/app/2060160/discussions/0/3806155895394113592/) で案内されている [公式 Wiki](https://thefarmerwasreplaced.wiki.gg/) を基準にしています。関数一覧は [Available Functions](https://thefarmerwasreplaced.wiki.gg/wiki/Available_Functions)、補完情報は [Tooltips Code](https://thefarmerwasreplaced.wiki.gg/wiki/Tooltips_Code)、Leaderboard 関連は [Leaderboard](https://thefarmerwasreplaced.wiki.gg/wiki/Leaderboard) を参照してください。

| 公式API | Rust入力 | 出力 | メモ |
| --- | --- | --- | --- |
| `harvest()` | `harvest();` | `harvest()` | そのまま |
| `can_harvest()` | `can_harvest()` | `can_harvest()` | 条件式で利用可能 |
| `swap(direction)` | `swap(Direction::North);` | `swap(North)` | 方向は名前空間なしに変換 |
| `plant(entity)` | `plant(Entity::Bush);` | `plant(Entities.Bush)` | 入力は単数 `Entity::X` |
| `move(direction)` | `move_dir(Direction::East);` | `move(East)` | 入力は `move_dir(Direction::X)` |
| `till()` | `till();` | `till()` | そのまま |
| `trade(item)` | `trade(Item::Carrot_Seed);` | `trade(Items.Carrot_Seed)` | 複数購入は `trade_n(Item::Carrot_Seed, 10);` |
| `get_pos_x()` | `get_pos_x()` | `get_pos_x()` | そのまま |
| `get_pos_y()` | `get_pos_y()` | `get_pos_y()` | そのまま |
| `get_world_size()` | `get_world_size()` | `get_world_size()` | そのまま |
| `get_entity_type()` | `get_entity_type()` | `get_entity_type()` | `Entity::X` との比較に使える |
| `get_ground_type()` | `get_ground_type()` | `get_ground_type()` | `Ground::Soil` との比較に使える |
| `get_tick_count()` | `get_tick_count()` | `get_tick_count()` | 実行 tick 計測 |
| `get_time()` | `get_time()` | `get_time()` | そのまま |
| `get_op_count()` | `get_op_count()` | `get_op_count()` | 公式では削除済み扱い。`get_tick_count()` 推奨 |
| `use_item(item, n=1)` | `use_item(Item::Fertilizer);` | `use_item(Items.Fertilizer)` | 回数指定は `use_item_n(Item::Fertilizer, 2);` |
| `get_water()` | `get_water()` | `get_water()` | そのまま |
| `do_a_flip()` | `do_a_flip();` | `do_a_flip()` | そのまま |
| `print(something)` | `print("soil");` | `print("soil")` | 値を画面に表示 |
| `quick_print(something)` | `quick_print("hi");` | `quick_print("hi")` | 軽い表示に使う |
| `len(collection)` | `len(xs)` | `len(xs)` | そのまま |
| `num_items(item)` | `num_items(Item::Fertilizer)` | `num_items(Items.Fertilizer)` | そのまま |
| `get_cost(thing)` | `get_cost(Unlock::Carrots)` | `get_cost(Unlocks.Carrots)` | そのまま |
| `clear()` | `clear();` | `clear()` | そのまま |
| `get_companion()` | `get_companion()` | `get_companion()` | 戻り値は変数に入れて添字アクセスできる |
| `unlock(unlock)` | `unlock(Unlock::Carrots);` | `unlock(Unlocks.Carrots)` | そのまま |
| `num_unlocked(thing)` | `num_unlocked(Unlock::Multi_Trade)` | `num_unlocked(Unlocks.Multi_Trade)` | そのまま |
| `timed_reset()` | `timed_reset();` | `timed_reset()` | そのまま |
| `measure()` | `measure();` | `measure()` | 方向指定は `measure_dir(Direction::North);` |
| `min(a,b)` | `min(a, b)` | `min(a, b)` | そのまま |
| `max(a,b)` | `max(a, b)` | `max(a, b)` | そのまま |
| `abs(number)` | `abs(x)` | `abs(x)` | そのまま |
| `random()` | `random()` | `random()` | そのまま |
| `list()` | `let xs = list();` / `let xs = [1, 2];` | `xs = list()` / `xs = [1, 2]` | list は関数またはRust風配列で作る |
| `set()` | `let seen = set();` | `seen = set()` | Python風 set リテラルは入力不可 |
| `dict()` | `let mut costs = dict(); costs[Item::Carrot_Seed] = 10;` | `costs = dict()` / `costs[Items.Carrot_Seed] = 10` | Python風 dict リテラルは入力不可 |
| `set_execution_speed(speed)` | `set_execution_speed(1);` | `set_execution_speed(1)` | そのまま |
| `set_farm_size(size)` | `set_farm_size(5);` | `set_farm_size(5)` | そのまま |
| `simulate(...)` | `simulate("main.py", [Unlock::Carrots], costs, globals, 0, 1);` | `simulate("main.py", [Unlocks.Carrots], costs, globals, 0, 1)` | dict引数は `dict()` と添字代入で用意 |
| `leaderboard_run(...)` | `leaderboard_run(Leaderboard::Fastest_Reset, filename, speedup);` | `leaderboard_run(Leaderboards.Fastest_Reset, filename, speedup)` | Leaderboard API |

## ゲーム内機能と対応範囲

| ゲーム内では使える要素 | Transplanter の現状 | メモ |
| --- | --- | --- |
| list / tuple リテラル | Rust版で対応 | `[1, 2]`、`(1, 2)` |
| dict / set リテラル | Rust版では入力拒否 | `dict()` / `set()` と添字代入を使う。Lisp版も `(dict)` / `(set)` を使う |
| 添字アクセス | 対応 | Rust版は `xs[i]`、Lisp版は `(index xs i)` / `(set-index! xs i value)` |
| collection への `for` | Rust版で対応 | `for item in xs { ... }` |
| デフォルト引数つき関数定義 | 入力では拒否 | RustやScheme風Lispの学習用入力から外れるため |
| ネストした関数定義 | Rust版で対応 | ブロック内の `fn` は Python のネスト `def` になる |
| ブロックコメント | Rust版で対応 | `/* ... */` は `# ...` へ変換。Lisp版は `;` 行コメント |
| Python風の `//` / `**` 演算子 | Rust版では入力拒否 | Rustの演算子として成立しないため |
| `simulate()` 用の複雑な dict/globals 指定 | 対応 | `dict()` で作って添字代入する |
| ゲーム内の unlock 状態・所持数・実行結果 | ゲーム内で確定 | 変換器は静的なコード生成だけを行う |
| ゲーム API の実際の挙動 | ゲーム内で実行 | `transplanter_rust::prelude` は Rust IDE 補完用の空実装 |

Rust の所有権・借用・ライフタイムの意味論はゲーム側には再現されません。ただし、選択中の言語モードで対象になる `.rs` は `cargo check` を通すので、Rustとして壊れた書き方は `.py` になる前に止まります。対象になる `.scm` / `.lisp` は Transplanter の Lisp パーサーと外部Scheme処理系で確認します。

## ソースからビルドする場合

開発者が確認する場合は、Windows PowerShellでこのリポジトリの `Cargo.toml` がある場所から実行します。

```powershell
cargo fmt --check
cargo build
cargo test
cargo build --release
.\target\release\transplanter.exe --help
.\target\release\transplanter.exe converters\rust_to_python\examples\basic.rs
.\target\release\transplanter.exe converters\rust_to_python\examples\basic.rs -o output.py
.\target\release\transplanter.exe converters\rust_to_python\examples\basic.rs --check
.\target\release\transplanter.exe converters\rust_to_python\examples\crop_columns.rs --check
.\target\release\transplanter.exe converters\lisp_to_python\examples\basic.scm
.\target\release\transplanter.exe converters\lisp_to_python\examples\basic.scm --check
.\target\release\transplanter.exe --sync
.\target\release\transplanter.exe --sync --language rust
.\target\release\transplanter.exe --sync --language lisp
.\target\release\transplanter.exe --init-ide
```

実プレイでは、上の検証コマンドよりも `Transplanter.exe` のダブルクリックGUIを使うのが簡単です。

## 開発者向けの構成

Transplanter本体と変換器は分けています。別言語の変換器を足す場合は、まず `converters/` に新しい crate を追加し、Transplanter本体側で `src/transplanter.rs` の `Converter` trait に接続する方針です。

```text
converters/
  lisp_to_python/                 Scheme風Lisp -> ゲーム用Python 変換器crate
    src/                          parser / codegen
    tests/                        変換器固有テスト
    examples/                     Lisp入力例
  rust_to_python/                 Rust -> ゲーム用Python 変換器crate
    src/                          変換ロジックと transplanter_rust prelude
    tests/                        変換器固有テスト
    examples/                     Rust入力例
src/
  main.rs                         起動だけ
  cli.rs                          コマンドライン引数と実行モード
  project.rs                      ソースフォルダから Save フォルダへの同期・監視
  rust_modules.rs                 mod 宣言と複数ファイル出力の判定
  rust_check.rs                   cargo check による Rust 検証
  lisp_check.rs                   Guile/Chez による Lisp 検証
  ide_support.rs                  transplanter_rust 補助crate生成
  paths.rs                        パス表示、toml文字列、既定フォルダ
  updater.rs                      GitHub Release の更新確認と差し替え準備
  win_gui.rs                      Windows GUI
  transplanter.rs                 変換器共通の trait
```

`transplanter_rust` は Rust -> Python 変換器 crate の名前であり、同時にユーザーの `.rs` をRustとして成立させるための prelude を提供します。ゲーム内APIの実行を再現する場所ではありません。変換ロジックは `converters/rust_to_python/` 側にあります。

`transplanter_lisp` は Scheme風Lisp -> Python 変換器 crate です。`.scm` / `.lisp` を読み、Transplanterの小さなLisp構文からゲーム用Pythonへ変換します。
