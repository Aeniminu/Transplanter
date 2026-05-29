use transplanter_lisp::{check_source, compile_source};

#[test]
fn basic_loop() {
    let source = r#"
(use transplanter)

(define (main)
  (harvest))
"#;

    assert_eq!(compile_source(source).unwrap(), "harvest()\n");
}

#[test]
fn helper_function_and_main_entry() {
    let source = r#"
(define (water)
  (if (< (get-water) 1.0)
      (use-item (item water))))

(define (main)
  (water)
  (plant (entity bush)))
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "def water():\n    if (get_water() < 1.0):\n        use_item(Items.Water)\n\nwater()\nplant(Entities.Bush)\n"
    );
}

#[test]
fn for_let_assignment_and_namespaces() {
    let source = r#"
(define (main)
  (let ((cycle #t))
    (loop
      (for i 0 4
        (if (can-harvest)
            (begin
              (harvest)
              (if cycle
                  (plant (entity tree))
                  (plant (entity bush)))))
        (set! cycle (not cycle))
        (move :north))
      (move :east))))
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "cycle = True\nwhile True:\n    for i in range(0, 4):\n        if can_harvest():\n            harvest()\n            if cycle:\n                plant(Entities.Tree)\n            else:\n                plant(Entities.Bush)\n        cycle = not cycle\n        move(North)\n    move(East)\n"
    );
}

#[test]
fn dict_indexing_and_simulate() {
    let source = r#"
(define (main)
  (let ((costs (dict)))
    (set-index! costs (item carrot-seed) 10)
    (simulate "main.py" (list (unlock carrots)) costs (dict) 0 1)))
"#;

    assert_eq!(
        compile_source(source).unwrap(),
        "costs = dict()\ncosts[Items.Carrot_Seed] = 10\nsimulate(\"main.py\", list(Unlocks.Carrots), costs, dict(), 0, 1)\n"
    );
}

#[test]
fn reports_unclosed_list_position() {
    let err = check_source("(define (main)\n  (harvest)\n").unwrap_err();
    assert_eq!(err.line, 1);
    assert_eq!(err.column, 1);
    assert!(err.message.contains("`)` が必要"));
}
