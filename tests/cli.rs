use std::fs;
use std::path::Path;
use std::process::Command;

const EXPECTED_BASIC: &str =
    "while True:\n    if can_harvest():\n        harvest()\n    else:\n        move(East)\n";

#[test]
fn example_basic_prints_expected_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("examples/basic.farmrs")
        .output()
        .expect("failed to run farmrs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), EXPECTED_BASIC);
}

#[test]
fn example_basic_writes_output_file() {
    let output_path = std::env::temp_dir().join(format!(
        "farmrs_cli_output_{}_{}.py",
        std::process::id(),
        unique_suffix()
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("examples/basic.farmrs")
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("failed to run farmrs");

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
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--version")
        .output()
        .expect("failed to run farmrs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = format!("farmrs {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn short_version_flag_prints_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("-V")
        .output()
        .expect("failed to run farmrs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = format!("farmrs {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn help_flag_prints_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--help")
        .output()
        .expect("failed to run farmrs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage:"), "stdout: {stdout}");
    assert!(
        stdout.contains("farmrs <input.rs|input.farmrs> --check"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("farmrs --init-ide [--src rs_src]"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("farmrs --sync [--src rs_src] [--out py_src]"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("farmrs --watch [--src rs_src] [--out py_src]"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("farmrs --version"), "stdout: {stdout}");
}

#[test]
fn unknown_option_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--unknown")
        .output()
        .expect("failed to run farmrs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: 不明なオプション `--unknown`"),
        "stderr: {stderr}"
    );
}

#[test]
fn missing_output_path_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("examples/basic.farmrs")
        .arg("-o")
        .output()
        .expect("failed to run farmrs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: -o/--output の後に出力パスが必要です"),
        "stderr: {stderr}"
    );
}

#[test]
fn version_with_extra_unknown_option_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--version")
        .arg("--unknown")
        .output()
        .expect("failed to run farmrs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: 不明なオプション `--unknown`"),
        "stderr: {stderr}"
    );
}

#[test]
fn help_with_extra_unknown_option_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--help")
        .arg("--unknown")
        .output()
        .expect("failed to run farmrs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: 不明なオプション `--unknown`"),
        "stderr: {stderr}"
    );
}

#[test]
fn check_flag_accepts_valid_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("examples/basic.farmrs")
        .arg("--check")
        .output()
        .expect("failed to run farmrs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "OK: examples/basic.farmrs\n"
    );
}

#[test]
fn check_flag_reports_japanese_file_position() {
    let input_path = std::env::temp_dir().join(format!(
        "farmrs_cli_invalid_{}_{}.farmrs",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(&input_path, "fn main() {\n    harvest()\n}\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg(&input_path)
        .arg("--check")
        .output()
        .expect("failed to run farmrs");

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
fn check_flag_accepts_trait_declarations() {
    let input_path = std::env::temp_dir().join(format!(
        "farmrs_cli_trait_{}_{}.farmrs",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(
        &input_path,
        "trait Tool {}\n\nfn main() {\n    harvest();\n}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg(&input_path)
        .arg("--check")
        .output()
        .expect("failed to run farmrs");

    let _ = fs::remove_file(&input_path);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_flag_rejects_output_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("examples/basic.farmrs")
        .arg("--check")
        .arg("-o")
        .arg("output.py")
        .output()
        .expect("failed to run farmrs");

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
        &workspace.join("rs_src").join("main.rs"),
        "fn main() {\n    harvest();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run farmrs");

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
        fs::read_to_string(workspace.join("rs_src").join("Cargo.toml"))
            .unwrap()
            .contains("path = \"main.rs\"")
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
        &workspace.join("rs_src").join("main.rs"),
        "use farmrs::prelude::*;\n\nfn main() {\n    harvest();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--init-ide")
        .current_dir(&workspace)
        .output()
        .expect("failed to run farmrs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = fs::read_to_string(workspace.join("rs_src").join("Cargo.toml")).unwrap();
    assert!(manifest.contains("[dependencies]"), "{manifest}");
    assert!(manifest.contains("farmrs = { path = "), "{manifest}");
    assert!(manifest.contains("[[bin]]"), "{manifest}");
    assert!(manifest.contains("name = \"main\""), "{manifest}");
    assert!(manifest.contains("path = \"main.rs\""), "{manifest}");

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn init_ide_manifest_passes_cargo_check() {
    let workspace = temp_workspace("init_ide_cargo_check");
    write_file(
        &workspace.join("rs_src").join("main.rs"),
        r#"use farmrs::prelude::*;

fn ready(entity: Entity) -> bool {
    if entity == Entity::Carrot {
        return can_harvest();
    }
    return false;
}

fn main() {
    /* block comments are fine for rust-analyzer and farmrs */
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

    let init_output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--init-ide")
        .current_dir(&workspace)
        .output()
        .expect("failed to run farmrs");
    assert!(
        init_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&init_output.stderr)
    );

    let check_output = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(workspace.join("rs_src").join("Cargo.toml"))
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
        "fn main() {\n    move_dir(Direction::East);\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--sync")
        .arg("--src")
        .arg("custom_rs")
        .arg("--out")
        .arg("custom_py")
        .current_dir(&workspace)
        .output()
        .expect("failed to run farmrs");

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
fn sync_keeps_subdirectory_layout() {
    let workspace = temp_workspace("sync_subdir");
    write_file(
        &workspace.join("rs_src").join("crops").join("carrot.rs"),
        "fn main() {\n    plant(Entity::Carrot);\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run farmrs");

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
fn sync_reports_japanese_file_position_for_invalid_source() {
    let workspace = temp_workspace("sync_invalid");
    write_file(
        &workspace.join("rs_src").join("bad.rs"),
        "fn main() {\n    harvest()\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run farmrs");

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
fn sync_still_accepts_farmrs_extension() {
    let workspace = temp_workspace("sync_farmrs_extension");
    write_file(
        &workspace.join("rs_src").join("legacy.farmrs"),
        "fn main() {\n    can_harvest();\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run farmrs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(workspace.join("py_src").join("legacy.py")).unwrap(),
        "can_harvest()\n"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn sync_rejects_input_file_mix() {
    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--sync")
        .arg("examples/basic.farmrs")
        .output()
        .expect("failed to run farmrs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: --sync/--watch と入力ファイルは同時に使えません"),
        "stderr: {stderr}"
    );
}

#[test]
fn sync_reports_missing_source_directory() {
    let workspace = temp_workspace("sync_missing_src");

    let output = Command::new(env!("CARGO_BIN_EXE_farmrs"))
        .arg("--sync")
        .current_dir(&workspace)
        .output()
        .expect("failed to run farmrs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("エラー: 入力フォルダ `rs_src` が見つかりません"),
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
        "farmrs_cli_{name}_{}_{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}
