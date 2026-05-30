use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_SRC_DIR: &str = "play_src";
pub const LEGACY_DEFAULT_SRC_DIR: &str = "rs_src";
pub const DEFAULT_OUT_DIR: &str = "py_src";
pub const SYSTEM_DIR: &str = ".transplanter";
pub const LEGACY_IDE_SUPPORT_DIR: &str = ".transplanter_ide";
pub const IDE_SUPPORT_CRATE_DIR: &str = "transplanter_rust";

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub trait CompileDiagnostic {
    fn line(&self) -> usize;
    fn column(&self) -> usize;
    fn message(&self) -> &str;
}

impl CompileDiagnostic for transplanter::error::RustToPythonError {
    fn line(&self) -> usize {
        self.line
    }

    fn column(&self) -> usize {
        self.column
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl CompileDiagnostic for transplanter::error::LispToPythonError {
    fn line(&self) -> usize {
        self.line
    }

    fn column(&self) -> usize {
        self.column
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub fn format_compile_error(path: &Path, err: impl CompileDiagnostic) -> String {
    format!(
        "エラー: {}:{}行{}列: {}",
        display_path(path),
        err.line(),
        err.column(),
        err.message()
    )
}

pub fn project_dir_for_src_dir(src_dir: &Path) -> PathBuf {
    src_dir
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn system_dir_for_src_dir(src_dir: &Path) -> PathBuf {
    project_dir_for_src_dir(src_dir).join(SYSTEM_DIR)
}

pub fn ensure_system_dir(system_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(system_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(system_dir)
        )
    })?;
    mark_hidden(system_dir);
    Ok(())
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
    has_extension(path, "rs")
}

pub fn is_lisp_file(path: &Path) -> bool {
    has_extension(path, "scm") || has_extension(path, "lisp")
}

pub fn is_source_file(path: &Path) -> bool {
    is_rs_file(path) || is_lisp_file(path)
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(expected))
}

pub fn should_skip_source_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name == SYSTEM_DIR || name == LEGACY_IDE_SUPPORT_DIR || name == "target"
        })
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

#[cfg(windows)]
fn mark_hidden(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_HIDDEN, GetFileAttributesW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW,
    };

    let path = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        let attributes = GetFileAttributesW(path.as_ptr());
        if attributes != INVALID_FILE_ATTRIBUTES && attributes & FILE_ATTRIBUTE_HIDDEN == 0 {
            let _ = SetFileAttributesW(path.as_ptr(), attributes | FILE_ATTRIBUTE_HIDDEN);
        }
    }
}

#[cfg(not(windows))]
fn mark_hidden(_path: &Path) {}
