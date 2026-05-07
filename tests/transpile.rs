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
fn syntax_error_for_missing_semicolon() {
    let source = r#"
fn main() {
    harvest()
}
"#;

    let err = compile_source(source).unwrap_err().to_string();
    assert!(
        err.contains("expected `;` after expression statement"),
        "{err}"
    );
    assert!(err.contains("at 4:1"), "{err}");
}

#[test]
fn unsupported_syntax_reports_clear_error() {
    let source = r#"
fn main() {
    trait Tool {}
}
"#;

    let err = compile_source(source).unwrap_err().to_string();
    assert!(err.contains("unsupported syntax: trait"), "{err}");
}

#[test]
fn block_comments_report_clear_error() {
    let source = r#"
fn main() {
    /* wait */
}
"#;

    let err = compile_source(source).unwrap_err().to_string();
    assert!(err.contains("unsupported syntax: block comments"), "{err}");
}

#[test]
fn generics_report_clear_error() {
    let source = r#"
fn helper<T>() {}

fn main() {}
"#;

    let err = compile_source(source).unwrap_err().to_string();
    assert!(err.contains("unsupported syntax: generics"), "{err}");
}
