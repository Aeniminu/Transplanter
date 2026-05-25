use farmrs::compile_source;

#[test]
fn basic_loop() {
    let source = r#"
fn main() {
    loop {
        if can_harvest() {
            harvest();
        } else {
            move_dir(Direction::East);
        }
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "while True:\n    if can_harvest():\n        harvest()\n    else:\n        move(East)\n"
    );
}

#[test]
fn let_and_if_else() {
    let source = r#"
fn main() {
    let mut x = 0;
    if x == 0 {
        plant(Entity::Bush);
    } else if x == 1 {
        plant(Entity::Carrot);
    } else {
        move_dir(Direction::North);
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "x = 0\nif x == 0:\n    plant(Entities.Bush)\nelif x == 1:\n    plant(Entities.Carrot)\nelse:\n    move(North)\n"
    );
}

#[test]
fn helper_function() {
    let source = r#"
fn clear_tile() {
    if can_harvest() {
        harvest();
    }
}

fn main() {
    clear_tile();
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "def clear_tile():\n    if can_harvest():\n        harvest()\n\nclear_tile()\n"
    );
}

#[test]
fn for_range() {
    let source = r#"
fn main() {
    for i in 0..10 {
        move_dir(Direction::East);
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "for i in range(10):\n    move(East)\n"
    );
}

#[test]
fn while_assignment_and_flow_control() {
    let source = r#"
fn main() {
    let mut x = 0;
    while x < 10 {
        x = x + 1;
        if x == 3 {
            continue;
        }
        if x == 8 {
            break;
        }
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "x = 0\nwhile x < 10:\n    x = x + 1\n    if x == 3:\n        continue\n    if x == 8:\n        break\n"
    );
}

#[test]
fn return_statement_in_helper_function() {
    let source = r#"
fn should_harvest() {
    if can_harvest() {
        return true;
    }
    return false;
}

fn main() {
    if should_harvest() {
        harvest();
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "def should_harvest():\n    if can_harvest():\n        return True\n    return False\n\nif should_harvest():\n    harvest()\n"
    );
}

#[test]
fn logical_operators_and_namespace_aliases() {
    let source = r#"
fn main() {
    if can_harvest() && !is_empty() || get_ground_type() == Ground::Soil {
        trade(Item::Carrot_Seed);
        plant(Entities::Bush);
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "if can_harvest() and not is_empty() or get_ground_type() == Grounds.Soil:\n    trade(Items.Carrot_Seed)\n    plant(Entities.Bush)\n"
    );
}

#[test]
fn game_api_namespaces_and_method_calls() {
    let source = r#"
fn main() {
    trade(Item::Carrot_Seed, 10);
    use_item(Item::Fertilizer);
    unlock(Unlock::Carrots);
    leaderboard_run(Leaderboard::Fastest_Reset, "reset.py", 10);
    xs.append(1);
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "trade(Items.Carrot_Seed, 10)\nuse_item(Items.Fertilizer)\nunlock(Unlocks.Carrots)\nleaderboard_run(Leaderboards.Fastest_Reset, \"reset.py\", 10)\nxs.append(1)\n"
    );
}

#[test]
fn prelude_import_is_ignored() {
    let source = r#"
use farmrs::prelude::*;

fn main() {
    harvest();
}
"#;

    assert_eq!(compile_source(source).unwrap(), "harvest()\n");
}

#[test]
fn general_use_statements_are_ignored() {
    let source = r#"
use std::mem;

fn main() {
    use std::cmp;
    harvest();
}
"#;

    assert_eq!(compile_source(source).unwrap(), "harvest()\n");
}

#[test]
fn typed_params_and_return_types_are_ignored_in_output() {
    let source = r#"
use farmrs::prelude::*;

fn should_harvest(entity: Entity) -> bool {
    if entity == Entity::Carrot {
        return true;
    }
    return false;
}

fn main() {
    if should_harvest(Entity::Carrot) {
        harvest();
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "def should_harvest(entity):\n    if entity == Entities.Carrot:\n        return True\n    return False\n\nif should_harvest(Entities.Carrot):\n    harvest()\n"
    );
}

#[test]
fn rust_friendly_api_aliases_are_rewritten() {
    let source = r#"
fn main() {
    trade_n(Item::Carrot_Seed, 10);
    use_item_n(Item::Fertilizer, 2);
    measure_dir(Direction::North);
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "trade(Items.Carrot_Seed, 10)\nuse_item(Items.Fertilizer, 2)\nmeasure(North)\n"
    );
}

#[test]
fn empty_block() {
    let source = r#"
fn main() {
    loop {
        // wait here
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "while True:\n    # wait here\n    pass\n"
    );
}

#[test]
fn comment_only_main_block() {
    let source = r#"
fn main() {
    // ready
}
"#;

    assert_eq!(compile_source(source).unwrap(), "# ready\npass\n");
}

#[test]
fn syntax_error_for_missing_semicolon() {
    let source = r#"
fn main() {
    harvest()
}
"#;

    let err = compile_source(source).unwrap_err().to_string();
    assert!(err.contains("式文の後に `;` が必要です"), "{err}");
    assert!(err.contains("4行1列"), "{err}");
}

#[test]
fn rust_metadata_items_are_lowered_or_ignored() {
    let source = r#"
trait Tool {
    fn run();
}

macro_rules! note {
    () => {};
}

struct Plan {
    entity: Entity,
    count: i32,
}

enum Crop {
    Carrot,
    Pumpkin,
}

impl Plan {
    fn make(entity: Entity, count: i32 = 1) -> Plan {
        return Plan { entity: entity, count: count };
    }
}

mod helpers {
    pub fn clear_tile() {
        harvest();
    }
}

fn main() {
    let plan = Plan { entity: Entity::Carrot, count: 10 };
    if Crop::Carrot == Crop::Carrot {
        helpers::clear_tile();
        Plan::make(plan["entity"]);
    }
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "Crop_Carrot = \"Crop.Carrot\"\nCrop_Pumpkin = \"Crop.Pumpkin\"\n\ndef Plan(entity=None, count=None):\n    return {\"entity\": entity, \"count\": count}\n\ndef Plan_make(entity, count=1):\n    return Plan(entity=entity, count=count)\n\ndef helpers_clear_tile():\n    harvest()\n\nplan = Plan(entity=Entities.Carrot, count=10)\nif Crop_Carrot == Crop_Carrot:\n    helpers_clear_tile()\n    Plan_make(plan[\"entity\"])\n"
    );
}

#[test]
fn block_comments_are_converted_to_python_comments() {
    let source = r#"
fn main() {
    /*
     wait
     */
}
"#;

    assert_eq!(compile_source(source).unwrap(), "#\n# wait\n#\npass\n");
}

#[test]
fn unclosed_block_comment_reports_clear_error() {
    let source = r#"
fn main() {
    /* wait
}
"#;

    let err = compile_source(source).unwrap_err().to_string();
    assert!(
        err.contains("ブロックコメントが閉じられていません"),
        "{err}"
    );
}

#[test]
fn function_generics_are_ignored_in_output() {
    let source = r#"
fn identity<T>(value: T) -> T {
    return value;
}

fn main() {
    identity(1);
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "def identity(value):\n    return value\n\nidentity(1)\n"
    );
}

#[test]
fn reference_and_lifetime_types_are_ignored_in_output() {
    let source = r#"
fn identity<'a>(value: &'a str) -> &'a str {
    return value;
}

fn main() {
    identity("seed");
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "def identity(value):\n    return value\n\nidentity(\"seed\")\n"
    );
}

#[test]
fn collection_literals_indexing_and_simulate_dicts() {
    let source = r#"
fn main() {
    let xs = [1, 2, 3];
    let pair = (1, 2);
    let costs = {Item::Carrot_Seed: 10, Item::Fertilizer: 1};
    xs[0] = xs[1] + costs[Item::Carrot_Seed];
    simulate("main.py", [Unlock::Carrots], {Item::Carrot_Seed: 10}, {"x": 1}, 0, 1);
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "xs = [1, 2, 3]\npair = (1, 2)\ncosts = {Items.Carrot_Seed: 10, Items.Fertilizer: 1}\nxs[0] = xs[1] + costs[Items.Carrot_Seed]\nsimulate(\"main.py\", [Unlocks.Carrots], {Items.Carrot_Seed: 10}, {\"x\": 1}, 0, 1)\n"
    );
}

#[test]
fn for_each_default_args_nested_functions_and_python_operators() {
    let source = r#"
fn walk(steps: i32 = 4) {
    fn step_once() {
        move_dir(Direction::East);
    }

    for item in [1, 2] {
        quick_print(item);
    }

    for item in {3, 4} {
        quick_print(item);
    }

    let half = steps // 2;
    let power = 2 ** 3;
    step_once();
}

fn main() {
    walk();
}
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "def walk(steps=4):\n    def step_once():\n        move(East)\n    for item in [1, 2]:\n        quick_print(item)\n    for item in {3, 4}:\n        quick_print(item)\n    half = steps // 2\n    power = 2 ** 3\n    step_once()\n\nwalk()\n"
    );
}
