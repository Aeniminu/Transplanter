use std::fs;
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

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos()
}
