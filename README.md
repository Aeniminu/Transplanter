# Transplanter / 耕訳機
※現在は、Windows専用です。

Transplanterは、Steamゲーム [The Farmer Was Replaced](https://store.steampowered.com/app/2060160/_Replace/?l=japanese) でPythonのみならず別の言語でもプログラミング学習を楽しむための、非公式変換アプリです。

現在入っている変換器は `Rust -> ゲーム用Python` のみです。
将来的には `Lisp -> Python` など別の変換器を `converters/` に追加できる構成にして行きたいなとか思っています。

※これはゲーム内のウィンドウで別言語を実行するためのツールではありません。主にIDEなどを用いながら別言語のコードを書いていただき、保存したらゲームで使用できるPython風コードに反映されるという形をとっています。


## 入手と初回セットアップ
1. 最新リリースの `Assets` から Windows 用の `Transplanter.exe` をダウンロードし、実プレイ用の作業フォルダへ置きます。
2.  `C:\Users\YourName\Desktop\farming` のようなお好きな作業フォルダを作ります。
3. その中に `Transplanter.exe` を置きます。
4. `Transplanter.exe` をダブルクリックします。
5. (自動)初回起動時に `transplanter.toml`、`Cargo.toml`、`rs_src/main.rs`、`.transplanter_ide/` などの必要なものが自動作成され、ウィンドウ上段の `rs_src のパス` が作業フォルダ内の `rs_src` を自動的に指定します。
6. ウィンドウ下段の `ゲームの Save フォルダ` に、ゲーム側のセーブフォルダを指定します。
7. 既に自動生成されているrs_src/main.rsなんかを編集・保存すると、ゲームに反映されているようになっているはずです。後はご自由にお楽しみください。

Saveフォルダが空欄の間は変換監視を開始しません。Saveフォルダを指定すると、`main.rs` を保存するたびに `cargo check` と変換が走ります。Rustとして間違っている場合、対応する `.py` は更新されません。
ここまで

## Save フォルダの見つけ方

[公式 Wiki の External Editor](https://thefarmerwasreplaced.wiki.gg/wiki/External_editor) では、外部エディタ用の Save フォルダはゲーム内の `Load` メニューにある `Open Folder` ボタンから開ける、と説明されています。

1. The Farmer Was Replaced を起動します。
2. `Load` メニューを開きます。
3. 使いたいセーブを選びます。
4. `Open Folder` を押します。
5. 開いたフォルダのパスを、Transplanter のウィンドウ下段に指定します。

開いた場所に `__builtins__.py` や既存の `.py` が見えていれば、そのフォルダが指定先です。`Saves` という親フォルダではなく、実際に `.py` が入っているセーブ個別のフォルダを指定してください。

## アップデート

Transplanter は起動時に GitHub Releases の最新版を確認します。今使っている exe より新しいリリースがある場合だけ、ウィンドウ上部に `更新しますか？` ボタンが表示されます。それ押せば更新されます。

作業フォルダの.exeを最新バージョンのexeファイルに差し替えることで、手動で行うこともできます。

## 以下は解説！

## どう動くか

実プレイ用の好きなフォルダを1つ作り、その中に `Transplanter.exe` を置いて起動して設定を完了すると、以下のようなプロセスを自動的に行います。

```text
好きな作業フォルダ/
  Transplanter.exe        exeファイル
  transplanter.toml       気にしなくていいやつ（パス設定。GUIで自動作成）
  Cargo.toml              Rust/Cursor補助用。自動作成
  .transplanter_ide/      Rust/Cursor補助用。自動作成
  rs_src/
    main.rs               あなたが編集するRustコード
    code0.rs            あなたが編集するRustコード2
↓
↓変換
↓
ゲームのSaveフォルダ/
  __builtins__.py         ゲーム側が作る補完用ファイル
  main.py                 Transplanterが出力するゲーム用コード
  code0.py              Transplanterが出力するゲーム用コード2
```

`rs_src` は自分が書く場所です。ゲームはここを直接読みません。ゲームが読むのは Save フォルダ内の `main.py` です。

## main.rs の最小例

```rust
use transplanter_rust::prelude::*;

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

`use transplanter_rust::prelude::*;` はCursorなどのIDEで未定義エラーを減らすための行です。変換後の `.py` には出力されません。

## ファイルの参照機能（ゲームで言うimport文）

`rs_src` の中では、Rustの `mod` を使うことで同じフォルダの別ファイルを参照できます。

```text
rs_src/
  main.rs
  code0.rs
```

`main.rs`:

```rust
use transplanter_rust::prelude::*;

mod code0;

fn main() {
    code0::main();
}
```

`code0.rs`:

```rust
use transplanter_rust::prelude::*;

pub fn main() {
    print("test_text");
}
```

出力は `main.py` と `code0.py` になります。`main.py` には `import code0` が入り、`code0::main();` は `code0.main()` になります。`code0.rs` 側の `pub fn main()` は自動実行されず、Pythonの `def main():` として出力されます。

今の対応範囲は、同じフォルダにある `mod code0;` -> `code0.rs` です。`mod.rs` 形式やサブフォルダ module は今後の拡張候補です。

## transplanter.toml の役割

`transplanter.toml` はTransplanterの設定メモです。GUIで選んだ2つのパスが保存されているだけです。

```toml
src_dir = "C:\\Users\\YourName\\Desktop\\farming\\rs_src"
out_dir = "C:\\Users\\YourName\\AppData\\LocalLow\\TheFarmerWasReplaced\\TheFarmerWasReplaced\\Saves\\Save0"
```

## IDE 補助

`rs_src/*.rs` はRustファイルなので、Cursorやrust-analyzerの補完を使えます。ただし `harvest()` や `Entity::Carrot` はゲーム独自APIなので、そのままだとRust側では未定義になります。

そのため、Transplanter は作業フォルダ直下に `Cargo.toml` と `.transplanter_ide/transplanter_rust/` を自動生成します。`rs_src` の中にはユーザーが読む・書く `.rs` だけを置く方針です。

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
.\Transplanter.exe --sync --src rs_src --out "ゲームのSaveフォルダ"
.\Transplanter.exe --watch --src rs_src --out "ゲームのSaveフォルダ"
.\Transplanter.exe rs_src\main.rs --check
```

`--out` を省略すると、既定では `py_src` に出力します。実プレイでは基本的にSaveフォルダを指定してください。削除された `.rs` に対応する `.py` は自動削除しません。ゲーム側で使っているファイルを誤って消さないため、不要な `.py` は手動で整理してください。


## 対応している主な構文:

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

## 主な変換ルール:

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

## 次のようなPython風・出力後形式は入力では使えません。

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

## ゲーム API 対応表

関数呼び出しは基本的にそのまま出力します。Transplanter が行うのは、Rustの名前空間や真偽値、演算子、範囲ループなどをゲーム用Pythonへ変換することです。ゲーム API が現在アンロック済みかどうか、ゲーム内で成功するかどうかは検証しません。

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
| list / tuple リテラル | 対応 | `[1, 2]`、`(1, 2)` |
| dict / set リテラル | 入力では拒否 | `dict()` / `set()` と添字代入を使う |
| 添字アクセス | 対応 | `xs[i]`、`costs[Item::Carrot_Seed]` の読み書き |
| collection への `for` | 対応 | `for item in xs { ... }` |
| デフォルト引数つき関数定義 | 入力では拒否 | Rustの関数定義として成立しないため |
| ネストした関数定義 | 対応 | ブロック内の `fn` は Python のネスト `def` になる |
| ブロックコメント | 対応 | `/* ... */` は `# ...` へ変換 |
| Python風の `//` / `**` 演算子 | 入力では拒否 | Rustの演算子として成立しないため |
| `simulate()` 用の複雑な dict/globals 指定 | 対応 | `dict()` で作って添字代入する |
| ゲーム内の unlock 状態・所持数・実行結果 | ゲーム内で確定 | 変換器は静的なコード生成だけを行う |
| ゲーム API の実際の挙動 | ゲーム内で実行 | `transplanter_rust::prelude` は IDE 補完用の空実装 |

Rust の所有権・借用・ライフタイムの意味論はゲーム側には再現されません。ただし、`cargo check` を通すので、Rustとして壊れた書き方は `.py` になる前に止まります。

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
.\target\release\transplanter.exe --sync
.\target\release\transplanter.exe --init-ide
```

実プレイでは、上の検証コマンドよりも `Transplanter.exe` のダブルクリックGUIを使うのが簡単です。

## 開発者向けの構成

Transplanter本体と変換器は分けています。別言語の変換器を足す場合は、まず `converters/` に新しい crate を追加し、Transplanter本体側で `src/transplanter.rs` の `Converter` trait に接続する方針です。

```text
converters/
  rust_to_python/                 Rust -> ゲーム用Python 変換器crate
    src/                          変換ロジックと transplanter_rust prelude
    tests/                        変換器固有テスト
    examples/                     Rust入力例
src/
  main.rs                         起動だけ
  cli.rs                          コマンドライン引数と実行モード
  project.rs                      rs_src から Save フォルダへの同期・監視
  rust_modules.rs                 mod 宣言と複数ファイル出力の判定
  rust_check.rs                   cargo check による Rust 検証
  ide_support.rs                  transplanter_rust 補助crate生成
  paths.rs                        パス表示、toml文字列、既定フォルダ
  updater.rs                      GitHub Release の更新確認と差し替え準備
  win_gui.rs                      Windows GUI
  transplanter.rs                 変換器共通の trait
```

`transplanter_rust` は Rust -> Python 変換器 crate の名前であり、同時にユーザーの `.rs` をRustとして成立させるための prelude を提供します。ゲーム内APIの実行を再現する場所ではありません。変換ロジックは `converters/rust_to_python/` 側にあります。
