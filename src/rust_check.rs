use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ide_support::{write_manifest_for_files, write_support_crate};
use crate::paths::{
    IDE_SUPPORT_CRATE_DIR, absolute_manifest_path, display_path, is_rs_file, manifest_path_string,
    toml_string,
};

pub fn validate_project_files(src_dir: &Path, rs_files: &[PathBuf]) -> Result<(), String> {
    if rs_files.is_empty() {
        return Ok(());
    }

    let manifest_path = write_manifest_for_files(src_dir, rs_files)?;
    run_cargo_check(&manifest_path)
}

pub fn validate_single_file(input_path: &Path) -> Result<(), String> {
    if !is_rs_file(input_path) {
        return Ok(());
    }

    if let Some(manifest_path) = find_nearest_transplanter_manifest(input_path) {
        return run_cargo_check(&manifest_path);
    }

    validate_single_file_with_temp_manifest(input_path)
}

fn validate_single_file_with_temp_manifest(input_path: &Path) -> Result<(), String> {
    let temp_dir = rust_validation_temp_dir();
    fs::create_dir_all(&temp_dir).map_err(|err| {
        format!(
            "エラー: Rust検証用フォルダ `{}` を作成できません: {err}",
            display_path(&temp_dir)
        )
    })?;

    let result = (|| {
        write_support_crate(&temp_dir)?;
        let manifest_path = temp_dir.join("Cargo.toml");
        fs::write(
            &manifest_path,
            render_single_file_validation_manifest(&absolute_manifest_path(input_path)),
        )
        .map_err(|err| {
            format!(
                "エラー: Rust検証用 manifest `{}` を作成できません: {err}",
                display_path(&manifest_path)
            )
        })?;
        run_cargo_check(&manifest_path)
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    result
}

fn render_single_file_validation_manifest(input_path: &Path) -> String {
    let mut manifest = String::new();
    manifest.push_str("[package]\n");
    manifest.push_str("name = \"transplanter-script-check\"\n");
    manifest.push_str("version = \"0.1.0\"\n");
    manifest.push_str("edition = \"2024\"\n");
    manifest.push_str("publish = false\n");
    manifest.push_str("autobins = false\n\n");
    manifest.push_str("[dependencies]\n");
    manifest.push_str(&format!(
        "transplanter_rust = {{ path = {} }}\n",
        toml_string(IDE_SUPPORT_CRATE_DIR)
    ));
    manifest.push_str("\n[[bin]]\n");
    manifest.push_str("name = \"script\"\n");
    manifest.push_str(&format!(
        "path = {}\n",
        toml_string(&manifest_path_string(input_path))
    ));
    manifest
}

fn find_nearest_transplanter_manifest(input_path: &Path) -> Option<PathBuf> {
    input_path.parent()?.ancestors().find_map(|dir| {
        let manifest_path = dir.join("Cargo.toml");
        if !manifest_path.is_file() {
            return None;
        }

        let manifest = fs::read_to_string(&manifest_path).ok()?;
        manifest
            .contains(IDE_SUPPORT_CRATE_DIR)
            .then_some(manifest_path)
    })
}

fn run_cargo_check(manifest_path: &Path) -> Result<(), String> {
    let output = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .arg("--color")
        .arg("never")
        .arg("--manifest-path")
        .arg(manifest_path)
        .current_dir(manifest_path.parent().unwrap_or_else(|| Path::new(".")))
        .output()
        .map_err(|err| {
            format!(
                "エラー: `.rs` をRustとして検証するための `cargo check` を実行できません: {err}\nRust/Cargo をインストールしてください"
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let details = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };

    Err(format!(
        "エラー: `.rs` がRustとしてコンパイルできません。\n{details}"
    ))
}

fn rust_validation_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!(
        "transplanter_rust_check_{}_{}",
        process::id(),
        nanos
    ))
}
