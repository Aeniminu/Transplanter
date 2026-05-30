use std::fs;
use std::path::Path;
use std::process::Command;

const EXPECTED_BASIC: &str = "harvest()\n";
const BASIC_EXAMPLE: &str = "converters/rust_to_python/examples/basic.rs";
const LISP_BASIC_EXAMPLE: &str = "converters/lisp_to_python/examples/basic.scm";

#[test]
fn example_basic_prints_expected_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(BASIC_EXAMPLE)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), EXPECTED_BASIC);
}

#[test]
fn example_lisp_basic_prints_expected_output() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_transplanter"));
    let fake_checker = add_fake_scheme_checker(&mut command);
    let output = command
        .arg(LISP_BASIC_EXAMPLE)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), EXPECTED_BASIC);
    let _ = fs::remove_dir_all(fake_checker);
}

#[test]
fn lisp_extension_writes_output_file() {
    let input_path = std::env::temp_dir().join(format!(
        "transplanter_cli_lisp_{}_{}.lisp",
        std::process::id(),
        unique_suffix()
    ));
    let output_path = input_path.with_extension("py");
    fs::write(
        &input_path,
        "(use transplanter)\n\n(define (main)\n  (quick-print \"lisp\"))\n",
    )
    .unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_transplanter"));
    let fake_checker = add_fake_scheme_checker(&mut command);
    let output = command
        .arg(&input_path)
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&output_path).unwrap(),
        "quick_print(\"lisp\")\n"
    );

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_dir_all(fake_checker);
}

#[test]
fn example_basic_writes_output_file() {
    let output_path = std::env::temp_dir().join(format!(
        "transplanter_cli_output_{}_{}.py",
        std::process::id(),
        unique_suffix()
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(BASIC_EXAMPLE)
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(&output_path).unwrap(), EXPECTED_BASIC);

    let _ = fs::remove_file(output_path);
}

#[test]
fn version_flag_prints_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--version")
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = format!("transplanter {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn short_version_flag_prints_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("-V")
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = format!("transplanter {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn help_flag_prints_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--help")
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage:"), "stdout: {stdout}");
    assert!(
        stdout.contains("transplanter <input.rs|input.scm|input.lisp> --check"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("transplanter --init-ide [--src play_src]"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("transplanter --sync [--src play_src] [--out py_src]"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("transplanter --watch [--src play_src] [--out py_src]"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("transplanter --version"),
        "stdout: {stdout}"
    );
}

#[test]
fn unknown_option_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--unknown")
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: 不明なオプション `--unknown`"),
        "stderr: {stderr}"
    );
}

#[test]
fn missing_output_path_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(BASIC_EXAMPLE)
        .arg("-o")
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: -o/--output の後に出力パスが必要です"),
        "stderr: {stderr}"
    );
}

#[test]
fn version_with_extra_unknown_option_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--version")
        .arg("--unknown")
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: 不明なオプション `--unknown`"),
        "stderr: {stderr}"
    );
}

#[test]
fn help_with_extra_unknown_option_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--help")
        .arg("--unknown")
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: 不明なオプション `--unknown`"),
        "stderr: {stderr}"
    );
}

#[test]
fn check_flag_accepts_valid_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(BASIC_EXAMPLE)
        .arg("--check")
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("OK: {BASIC_EXAMPLE}\n")
    );
}

#[test]
fn check_flag_accepts_lisp_input() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_transplanter"));
    let fake_checker = add_fake_scheme_checker(&mut command);
    let output = command
        .arg(LISP_BASIC_EXAMPLE)
        .arg("--check")
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("OK: {LISP_BASIC_EXAMPLE}\n")
    );
    let _ = fs::remove_dir_all(fake_checker);
}

#[test]
fn check_flag_accepts_lisp_with_guile_command() {
    let workspace = temp_workspace("guile_command_checker");
    let input_path = workspace.join("main.scm");
    write_file(
        &input_path,
        "(use transplanter)\n\n(define (main)\n  (harvest))\n",
    );
    let checker_path = write_fake_guile(&workspace);

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(&input_path)
        .arg("--check")
        .env("PATH", &workspace)
        .env_remove("TRANSPLANTER_GUILE_GUILD")
        .env("TRANSPLANTER_GUILE", checker_path)
        .env_remove("TRANSPLANTER_CHEZ_SCHEME")
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn check_flag_reports_lisp_file_position() {
    let input_path = std::env::temp_dir().join(format!(
        "transplanter_cli_invalid_{}_{}.scm",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(&input_path, "(define (main)\n  (harvest)\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(&input_path)
        .arg("--check")
        .output()
        .expect("failed to run transplanter");

    let _ = fs::remove_file(&input_path);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(&input_path.to_string_lossy().to_string()),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("1行1列"), "stderr: {stderr}");
    assert!(stderr.contains("`)` が必要"), "stderr: {stderr}");
}

#[test]
fn check_flag_accepts_lisp_without_external_scheme_checker() {
    let workspace = temp_workspace("missing_scheme_checker");
    let input_path = workspace.join("main.scm");
    write_file(
        &input_path,
        "(use transplanter)\n\n(define (main)\n  (harvest))\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(&input_path)
        .arg("--check")
        .env("PATH", &workspace)
        .env_remove("TRANSPLANTER_GUILE_GUILD")
        .env_remove("TRANSPLANTER_GUILE")
        .env_remove("TRANSPLANTER_CHEZ_SCHEME")
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("OK: {}\n", input_path.to_string_lossy())
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn check_flag_reports_japanese_file_position() {
    let input_path = std::env::temp_dir().join(format!(
        "transplanter_cli_invalid_{}_{}.rs",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(&input_path, "fn main() {\n    harvest()\n}\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(&input_path)
        .arg("--check")
        .output()
        .expect("failed to run transplanter");

    let _ = fs::remove_file(&input_path);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(&input_path.to_string_lossy().to_string()),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("3行1列"), "stderr: {stderr}");
    assert!(
        stderr.contains("式文の後に `;` が必要です"),
        "stderr: {stderr}"
    );
}

#[test]
fn check_flag_rejects_python_output_syntax() {
    let input_path = std::env::temp_dir().join(format!(
        "transplanter_cli_python_syntax_{}_{}.rs",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(
        &input_path,
        "fn main() {\n    for i in range(4) {\n        move(North);\n    }\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(&input_path)
        .arg("--check")
        .output()
        .expect("failed to run transplanter");

    let _ = fs::remove_file(&input_path);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(&input_path.to_string_lossy().to_string()),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("`range(...)` は入力では使えません"),
        "stderr: {stderr}"
    );
}

#[test]
fn check_flag_rejects_rs_that_fails_cargo_check() {
    let input_path = std::env::temp_dir().join(format!(
        "transplanter_cli_rust_invalid_{}_{}.rs",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(
        &input_path,
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n    missing_game_api();\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(&input_path)
        .arg("--check")
        .output()
        .expect("failed to run transplanter");

    let _ = fs::remove_file(&input_path);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("`.rs` がRustとしてコンパイルできません"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("missing_game_api"), "stderr: {stderr}");
}

#[test]
fn check_flag_accepts_trait_declarations() {
    let input_path = std::env::temp_dir().join(format!(
        "transplanter_cli_trait_{}_{}.rs",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(
        &input_path,
        "use transplanter_rust::prelude::*;\n\ntrait Tool {}\n\nfn main() {\n    harvest();\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(&input_path)
        .arg("--check")
        .output()
        .expect("failed to run transplanter");

    let _ = fs::remove_file(&input_path);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_flag_rejects_output_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(BASIC_EXAMPLE)
        .arg("--check")
        .arg("-o")
        .arg("output.py")
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: --check と -o/--output は同時に使えません"),
        "stderr: {stderr}"
    );
}

#[test]
fn sync_uses_default_directories() {
    let workspace = temp_workspace("sync_default");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("main.py")).unwrap(),
        "harvest()\n"
    );
    assert!(
        fs::read_to_string(transplanter_manifest(&workspace))
            .unwrap()
            .contains("path = \"../play_src/main.rs\"")
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("OK: 1 件"),
        "stdout did not contain sync count"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn init_ide_generates_manifest() {
    let workspace = temp_workspace("init_ide");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--init-ide")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = fs::read_to_string(transplanter_manifest(&workspace)).unwrap();
    assert!(manifest.contains("[dependencies]"), "{manifest}");
    assert!(
        manifest.contains("transplanter_rust = { path = \"transplanter_rust\" }"),
        "{manifest}"
    );
    assert!(manifest.contains("[[bin]]"), "{manifest}");
    assert!(manifest.contains("name = \"main\""), "{manifest}");
    assert!(
        manifest.contains("path = \"../play_src/main.rs\""),
        "{manifest}"
    );
    assert!(
        workspace
            .join(".transplanter")
            .join("transplanter_rust")
            .join("src")
            .join("prelude.rs")
            .is_file()
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn init_ide_manifest_passes_cargo_check() {
    let workspace = temp_workspace("init_ide_cargo_check");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        r#"use transplanter_rust::prelude::*;

fn ready(entity: Entity) -> bool {
    if entity == Entity::Carrot {
        return can_harvest();
    }
    return false;
}

fn main() {
    /* block comments are fine for rust-analyzer and Transplanter */
    let mut xs = [1, 2, 3];
    xs[0] = xs[1];
    for item in xs {
        quick_print(item);
    }

    let mut costs = dict::<Item, i32>();
    costs[Item::Carrot_Seed] = 10;
    simulate("main.py", [Unlock::Carrots], costs, (), 0, 1);

    if ready(Entity::Carrot) {
        harvest();
        trade_n(Item::Carrot_Seed, 10);
        use_item_n(Item::Fertilizer, 2);
        measure_dir(Direction::North);
    }
}
"#,
    );

    let init_output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--init-ide")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");
    assert!(
        init_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&init_output.stderr)
    );

    let check_output = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(transplanter_manifest(&workspace))
        .output()
        .expect("failed to run cargo check");

    assert!(
        check_output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&check_output.stdout),
        String::from_utf8_lossy(&check_output.stderr)
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_accepts_custom_directories() {
    let workspace = temp_workspace("sync_custom");
    write_file(
        &workspace.join("custom_rs").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    move_dir(Direction::East);\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .arg("--src")
        .arg("custom_rs")
        .arg("--out")
        .arg("custom_py")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("custom_py").join("main.py")).unwrap(),
        "move(East)\n"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_accepts_legacy_rs_src_when_explicit() {
    let workspace = temp_workspace("sync_legacy_rs_src");
    write_file(
        &workspace.join("rs_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .arg("--src")
        .arg("rs_src")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("main.py")).unwrap(),
        "harvest()\n"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_accepts_lisp_sources() {
    let workspace = temp_workspace("sync_lisp");
    write_file(
        &workspace.join("play_src").join("main.scm"),
        "(use transplanter)\n\n(define (main)\n  (clear)\n  (move :east))\n",
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_transplanter"));
    let fake_checker = add_fake_scheme_checker(&mut command);
    let output = command
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("main.py")).unwrap(),
        "clear()\nmove(East)\n"
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_dir_all(fake_checker);
}

#[test]
fn sync_accepts_mixed_rust_and_lisp_sources() {
    let workspace = temp_workspace("sync_mixed");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n}\n",
    );
    write_file(
        &workspace.join("play_src").join("lab.scm"),
        "(define (main)\n  (quick-print \"lab\"))\n",
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_transplanter"));
    let fake_checker = add_fake_scheme_checker(&mut command);
    let output = command
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("main.py")).unwrap(),
        "harvest()\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("lab.py")).unwrap(),
        "quick_print(\"lab\")\n"
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_dir_all(fake_checker);
}

#[test]
fn sync_rust_language_ignores_lisp_sources() {
    let workspace = temp_workspace("sync_rust_language");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n}\n",
    );
    write_file(
        &workspace.join("play_src").join("main.scm"),
        "(not valid lisp",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .arg("--language")
        .arg("rust")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("main.py")).unwrap(),
        "harvest()\n"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_lisp_language_ignores_rust_sources() {
    let workspace = temp_workspace("sync_lisp_language");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    missing_game_api();\n}\n",
    );
    write_file(
        &workspace.join("play_src").join("main.scm"),
        "(use transplanter)\n\n(define (main)\n  (harvest))\n",
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_transplanter"));
    let fake_checker = add_fake_scheme_checker(&mut command);
    let output = command
        .arg("--sync")
        .arg("--language")
        .arg("lisp")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("main.py")).unwrap(),
        "harvest()\n"
    );

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_dir_all(fake_checker);
}

#[test]
fn sync_rejects_duplicate_output_paths_across_languages() {
    let workspace = temp_workspace("sync_duplicate_outputs");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n}\n",
    );
    write_file(
        &workspace.join("play_src").join("main.scm"),
        "(define (main)\n  (quick-print \"lisp\"))\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("main.rs"), "stderr: {stderr}");
    assert!(stderr.contains("main.scm"), "stderr: {stderr}");
    assert!(stderr.contains("main.py"), "stderr: {stderr}");
    assert!(stderr.contains("出力先"), "stderr: {stderr}");

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_auto_language_rejects_duplicate_output_paths() {
    let workspace = temp_workspace("sync_auto_duplicate_outputs");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n}\n",
    );
    write_file(
        &workspace.join("play_src").join("main.scm"),
        "(define (main)\n  (quick-print \"lisp\"))\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .arg("--language")
        .arg("auto")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("main.rs"), "stderr: {stderr}");
    assert!(stderr.contains("main.scm"), "stderr: {stderr}");
    assert!(stderr.contains("main.py"), "stderr: {stderr}");
    assert!(stderr.contains("出力先"), "stderr: {stderr}");

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_keeps_subdirectory_layout() {
    let workspace = temp_workspace("sync_subdir");
    write_file(
        &workspace.join("play_src").join("crops").join("carrot.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    plant(Entity::Carrot);\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("crops").join("carrot.py")).unwrap(),
        "plant(Entities.Carrot)\n"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_outputs_external_module_as_importable_python_file() {
    let workspace = temp_workspace("sync_external_module");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nmod farmlab;\n\nfn main() {\n    farmlab::main();\n}\n",
    );
    write_file(
        &workspace.join("play_src").join("farmlab.rs"),
        "use transplanter_rust::prelude::*;\n\npub fn main() {\n    print(\"test_text\");\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("main.py")).unwrap(),
        "import farmlab\n\nfarmlab.main()\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("farmlab.py")).unwrap(),
        "def main():\n    print(\"test_text\")\n"
    );
    assert!(
        !fs::read_to_string(transplanter_manifest(&workspace))
            .unwrap()
            .contains("path = \"../play_src/farmlab.rs\"")
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_updates_external_module_as_module_file() {
    let workspace = temp_workspace("sync_external_module_update");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nmod farmlab;\n\nfn main() {\n    farmlab::main();\n}\n",
    );
    let module_path = workspace.join("play_src").join("farmlab.rs");
    write_file(
        &module_path,
        "use transplanter_rust::prelude::*;\n\npub fn main() {\n    print(\"first\");\n}\n",
    );

    let first = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    write_file(
        &module_path,
        "use transplanter_rust::prelude::*;\n\npub fn main() {\n    print(\"second\");\n}\n",
    );
    let second = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("farmlab.py")).unwrap(),
        "def main():\n    print(\"second\")\n"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_reports_japanese_file_position_for_invalid_source() {
    let workspace = temp_workspace("sync_invalid");
    write_file(
        &workspace.join("play_src").join("bad.rs"),
        "fn main() {\n    harvest()\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("bad.rs"), "stderr: {stderr}");
    assert!(stderr.contains("3行1列"), "stderr: {stderr}");
    assert!(
        stderr.contains("式文の後に `;` が必要です"),
        "stderr: {stderr}"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_reports_missing_external_module_file() {
    let workspace = temp_workspace("sync_missing_external_module");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nmod missing;\n\nfn main() {\n    missing::main();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("main.rs"), "stderr: {stderr}");
    assert!(stderr.contains("mod missing;"), "stderr: {stderr}");
    assert!(stderr.contains("missing.rs"), "stderr: {stderr}");

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_rejects_rs_that_fails_cargo_check_without_writing_output() {
    let workspace = temp_workspace("sync_rust_invalid");
    write_file(
        &workspace.join("play_src").join("main.rs"),
        "use transplanter_rust::prelude::*;\n\nfn main() {\n    harvest();\n    missing_game_api();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("`.rs` がRustとしてコンパイルできません"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("missing_game_api"), "stderr: {stderr}");
    assert!(
        !workspace.join("py_src").join("main.py").exists(),
        "invalid Rust should not produce py output"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_ignores_non_rs_extension() {
    let workspace = temp_workspace("sync_non_rs_extension");
    write_file(
        &workspace.join("play_src").join("notes.txt"),
        "fn main() {\n    can_harvest();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!workspace.join("py_src").join("notes.py").exists());

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_rejects_input_file_mix() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .arg(BASIC_EXAMPLE)
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: --sync/--watch と入力ファイルは同時に使えません"),
        "stderr: {stderr}"
    );
}

#[test]
fn sync_rejects_unknown_language_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .arg("--language")
        .arg("ruby")
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--language"), "stderr: {stderr}");
    assert!(stderr.contains("auto"), "stderr: {stderr}");
    assert!(stderr.contains("rust"), "stderr: {stderr}");
    assert!(stderr.contains("lisp"), "stderr: {stderr}");
}

#[test]
fn single_file_mode_rejects_language_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg(BASIC_EXAMPLE)
        .arg("--language")
        .arg("rust")
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("単体ファイル変換では --language は使いません"),
        "stderr: {stderr}"
    );
}

#[test]
fn sync_reports_missing_source_directory() {
    let workspace = temp_workspace("sync_missing_src");

    let output = Command::new(env!("CARGO_BIN_EXE_transplanter"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run transplanter");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: 入力フォルダ `play_src` が見つかりません"),
        "stderr: {stderr}"
    );

    let _ = fs::remove_dir_all(workspace);
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos()
}

fn temp_workspace(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "transplanter_cli_{name}_{}_{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn add_fake_scheme_checker(command: &mut Command) -> std::path::PathBuf {
    let dir = temp_workspace("fake_scheme_checker");
    let checker_path = write_fake_guild(&dir);
    command.env("PATH", path_with_prepended(&dir));
    command.env("TRANSPLANTER_GUILE_GUILD", checker_path);
    dir
}

fn path_with_prepended(dir: &Path) -> std::ffi::OsString {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(current_path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current_path));
    }
    std::env::join_paths(paths).unwrap()
}

fn transplanter_manifest(workspace: &Path) -> std::path::PathBuf {
    workspace.join(".transplanter").join("Cargo.toml")
}

#[cfg(windows)]
fn write_fake_guild(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("guild.cmd");
    write_file(&path, "@echo off\r\nexit /b 0\r\n");
    path
}

#[cfg(windows)]
fn write_fake_guile(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("guile.cmd");
    write_file(&path, "@echo off\r\nexit /b 0\r\n");
    path
}

#[cfg(not(windows))]
fn write_fake_guild(dir: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("guild");
    write_file(&path, "#!/bin/sh\nexit 0\n");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

#[cfg(not(windows))]
fn write_fake_guile(dir: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("guile");
    write_file(&path, "#!/bin/sh\nexit 0\n");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}
