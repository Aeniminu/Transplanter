use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::ide_support::write_manifest_for_files;
use crate::language::LanguageMode;
use crate::lisp_check::validate_lisp_file;
use crate::paths::{
    display_path, ensure_source_dir, format_compile_error, is_lisp_file, is_rs_file,
    should_skip_source_dir,
};
use crate::rust_check::validate_project_files;
use crate::rust_modules::discover_module_files;

const WATCH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileStamp {
    modified: SystemTime,
    len: u64,
}

pub fn sync_project(
    src_dir: &Path,
    out_dir: &Path,
    language: LanguageMode,
) -> Result<usize, String> {
    ensure_source_dir(src_dir)?;
    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(out_dir)
        )
    })?;

    let files = find_source_files(src_dir, language)?;
    ensure_unique_output_paths(src_dir, out_dir, &files)?;
    let rs_files = rust_source_files(&files);
    let module_files = discover_module_files(&rs_files)?;
    for input_path in &files {
        check_project_file(input_path, module_files.contains(input_path))?;
    }
    validate_project_files(src_dir, &rs_files)?;
    for input_path in &files {
        compile_project_file_unchecked(
            src_dir,
            out_dir,
            input_path,
            module_files.contains(input_path),
        )?;
    }

    Ok(files.len())
}

pub fn watch_project(src_dir: &Path, out_dir: &Path, language: LanguageMode) -> Result<(), String> {
    let count = sync_project(src_dir, out_dir, language)?;
    println!(
        "OK: {} 件を {} から {} へ変換しました",
        count,
        display_path(src_dir),
        display_path(out_dir)
    );
    println!("watch: ソースファイルの変更を監視しています。終了するには Ctrl+C を押してください。");

    let mut seen = snapshot_source_files(src_dir, language)?;
    let mut seen_outputs = snapshot_output_files(src_dir, out_dir, seen.keys())?;
    loop {
        thread::sleep(WATCH_INTERVAL);
        let current = snapshot_source_files(src_dir, language)?;
        let current_outputs = snapshot_output_files(src_dir, out_dir, current.keys())?;

        if current.keys().ne(seen.keys()) {
            write_manifest_for_language(src_dir, language, current.keys())?;
        }

        let source_changed = current
            .iter()
            .any(|(input_path, stamp)| seen.get(input_path) != Some(stamp));
        if current.keys().ne(seen.keys()) || source_changed {
            let count = sync_project(src_dir, out_dir, language)?;
            println!("OK: {count} 件を再同期しました");
            seen = current;
            seen_outputs = snapshot_output_files(src_dir, out_dir, seen.keys())?;
            continue;
        }

        for input_path in current.keys() {
            let output_path = output_path_for(src_dir, out_dir, input_path)?;
            let output_changed =
                seen_outputs.get(&output_path) != current_outputs.get(&output_path);
            if output_changed {
                let output_path = compile_project_file(src_dir, out_dir, input_path, language)?;
                println!("OK: {} を変換しました", display_path(&output_path));
            }
        }

        seen = current;
        seen_outputs = snapshot_output_files(src_dir, out_dir, seen.keys())?;
    }
}

pub fn compile_project_file(
    src_dir: &Path,
    out_dir: &Path,
    input_path: &Path,
    language: LanguageMode,
) -> Result<PathBuf, String> {
    if !language.accepts_path(input_path) {
        return Err(format!(
            "エラー: `{}` は {} mode の対象ファイルではありません",
            display_path(input_path),
            language.as_str()
        ));
    }

    let files = find_source_files(src_dir, language)?;
    ensure_unique_output_paths(src_dir, out_dir, &files)?;
    let rs_files = rust_source_files(&files);
    let module_files = discover_module_files(&rs_files)?;
    let output = compile_project_source(input_path, module_files.contains(input_path))?;

    if is_rs_file(input_path) {
        validate_project_files(src_dir, &rs_files)?;
    }
    if is_lisp_file(input_path) {
        validate_lisp_file(input_path)?;
    }

    write_project_output(src_dir, out_dir, input_path, output)
}

pub fn output_path_for(
    src_dir: &Path,
    out_dir: &Path,
    input_path: &Path,
) -> Result<PathBuf, String> {
    let relative = input_path.strip_prefix(src_dir).map_err(|_| {
        format!(
            "エラー: `{}` は `{}` の中にありません",
            display_path(input_path),
            display_path(src_dir)
        )
    })?;
    let mut output_path = out_dir.join(relative);
    output_path.set_extension("py");
    Ok(output_path)
}

pub fn snapshot_source_files(
    src_dir: &Path,
    language: LanguageMode,
) -> Result<BTreeMap<PathBuf, FileStamp>, String> {
    ensure_source_dir(src_dir)?;
    let mut snapshot = BTreeMap::new();

    for file in find_source_files(src_dir, language)? {
        snapshot.insert(file.clone(), file_stamp(&file)?);
    }

    Ok(snapshot)
}

pub fn snapshot_output_files<'a>(
    src_dir: &Path,
    out_dir: &Path,
    input_paths: impl Iterator<Item = &'a PathBuf>,
) -> Result<BTreeMap<PathBuf, Option<FileStamp>>, String> {
    let mut snapshot = BTreeMap::new();

    for input_path in input_paths {
        let output_path = output_path_for(src_dir, out_dir, input_path)?;
        snapshot.insert(output_path.clone(), file_stamp(&output_path).ok());
    }

    Ok(snapshot)
}

fn compile_project_file_unchecked(
    src_dir: &Path,
    out_dir: &Path,
    input_path: &Path,
    is_module: bool,
) -> Result<PathBuf, String> {
    let output = compile_project_source(input_path, is_module)?;
    write_project_output(src_dir, out_dir, input_path, output)
}

fn check_project_file(input_path: &Path, is_module: bool) -> Result<(), String> {
    let source = read_project_source(input_path)?;
    if is_rs_file(input_path) {
        return if is_module {
            transplanter::check_module_source(&source)
        } else {
            transplanter::check_source(&source)
        }
        .map_err(|err| format_compile_error(input_path, err));
    }
    if is_lisp_file(input_path) {
        transplanter::check_lisp_source(&source)
            .map_err(|err| format_compile_error(input_path, err))?;
        return validate_lisp_file(input_path);
    }
    Err(format!(
        "エラー: `{}` は対応している入力ファイルではありません",
        display_path(input_path)
    ))
}

fn compile_project_source(input_path: &Path, is_module: bool) -> Result<String, String> {
    let source = read_project_source(input_path)?;
    if is_rs_file(input_path) {
        return if is_module {
            transplanter::compile_module_source(&source)
        } else {
            transplanter::compile_source(&source)
        }
        .map_err(|err| format_compile_error(input_path, err));
    }
    if is_lisp_file(input_path) {
        return transplanter::compile_lisp_source(&source)
            .map_err(|err| format_compile_error(input_path, err));
    }
    Err(format!(
        "エラー: `{}` は対応している入力ファイルではありません",
        display_path(input_path)
    ))
}

fn read_project_source(input_path: &Path) -> Result<String, String> {
    fs::read_to_string(input_path).map_err(|err| {
        format!(
            "エラー: `{}` を読み込めません: {err}",
            display_path(input_path)
        )
    })
}

fn write_project_output(
    src_dir: &Path,
    out_dir: &Path,
    input_path: &Path,
    output: String,
) -> Result<PathBuf, String> {
    let output_path = output_path_for(src_dir, out_dir, input_path)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("エラー: `{}` を作成できません: {err}", display_path(parent)))?;
    }

    fs::write(&output_path, output).map_err(|err| {
        format!(
            "エラー: `{}` に書き込めません: {err}",
            display_path(&output_path)
        )
    })?;

    Ok(output_path)
}

fn ensure_unique_output_paths(
    src_dir: &Path,
    out_dir: &Path,
    files: &[PathBuf],
) -> Result<(), String> {
    let mut outputs = BTreeMap::new();
    for input_path in files {
        let output_path = output_path_for(src_dir, out_dir, input_path)?;
        if let Some(previous) = outputs.insert(output_path.clone(), input_path.clone()) {
            return Err(format!(
                "エラー: `{}` と `{}` の出力先がどちらも `{}` になります。どちらかのファイル名を変更してください",
                display_path(&previous),
                display_path(input_path),
                display_path(&output_path)
            ));
        }
    }
    Ok(())
}

fn file_stamp(path: &Path) -> Result<FileStamp, String> {
    let metadata = fs::metadata(path)
        .map_err(|err| format!("エラー: `{}` を確認できません: {err}", display_path(path)))?;
    let modified = metadata
        .modified()
        .map_err(|err| format!("エラー: `{}` を確認できません: {err}", display_path(path)))?;
    Ok(FileStamp {
        modified,
        len: metadata.len(),
    })
}

fn find_source_files(src_dir: &Path, language: LanguageMode) -> Result<Vec<PathBuf>, String> {
    ensure_source_dir(src_dir)?;
    let mut files = Vec::new();
    collect_source_files(src_dir, language, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_source_files(
    dir: &Path,
    language: LanguageMode,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(dir)
        .map_err(|err| format!("エラー: `{}` を読み込めません: {err}", display_path(dir)))?
    {
        let entry = entry
            .map_err(|err| format!("エラー: `{}` を読み込めません: {err}", display_path(dir)))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|err| format!("エラー: `{}` を確認できません: {err}", display_path(&path)))?;

        if metadata.is_dir() && !should_skip_source_dir(&path) {
            collect_source_files(&path, language, files)?;
        } else if metadata.is_file() && language.accepts_path(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn rust_source_files(files: &[PathBuf]) -> Vec<PathBuf> {
    files
        .iter()
        .filter(|path| is_rs_file(path))
        .cloned()
        .collect()
}

fn write_manifest_for_language<'a>(
    src_dir: &Path,
    language: LanguageMode,
    input_paths: impl Iterator<Item = &'a PathBuf>,
) -> Result<(), String> {
    if !language.includes_rust() {
        return Ok(());
    }

    let rs_files = input_paths
        .filter(|path| is_rs_file(path))
        .cloned()
        .collect::<Vec<_>>();
    write_manifest_for_files(src_dir, &rs_files).map(|_| ())
}
