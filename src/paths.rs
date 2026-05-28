use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_SRC_DIR: &str = "rs_src";
pub const DEFAULT_OUT_DIR: &str = "py_src";
pub const IDE_SUPPORT_DIR: &str = ".transplanter_ide";
pub const IDE_SUPPORT_CRATE_DIR: &str = ".transplanter_ide/transplanter_rust";

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn format_compile_error(path: &Path, err: transplanter::error::RustToPythonError) -> String {
    format!(
        "エラー: {}:{}行{}列: {}",
        display_path(path),
        err.line,
        err.column,
        err.message
    )
}

pub fn project_dir_for_src_dir(src_dir: &Path) -> PathBuf {
    src_dir
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn relative_path_for_manifest(relative: &Path) -> String {
    relative.to_string_lossy().replace('\\', "/")
}

pub fn manifest_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn absolute_manifest_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

pub fn toml_string(value: &str) -> String {
    let mut output = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            _ => output.push(ch),
        }
    }
    output.push('"');
    output
}

pub fn is_rs_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "rs")
}

pub fn should_skip_source_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == IDE_SUPPORT_DIR || name == "target")
}

pub fn ensure_source_dir(src_dir: &Path) -> Result<(), String> {
    match fs::metadata(src_dir) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(format!(
            "エラー: `{}` はフォルダではありません",
            display_path(src_dir)
        )),
        Err(err) => Err(format!(
            "エラー: 入力フォルダ `{}` が見つかりません: {err}",
            display_path(src_dir)
        )),
    }
}
