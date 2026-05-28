use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::ide_support::write_manifest;
use crate::paths::{
    display_path, ensure_source_dir, format_compile_error, is_rs_file, should_skip_source_dir,
};
use crate::rust_check::validate_project;

const WATCH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileStamp {
    modified: SystemTime,
    len: u64,
}

pub fn sync_project(src_dir: &Path, out_dir: &Path) -> Result<usize, String> {
    ensure_source_dir(src_dir)?;
    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(out_dir)
        )
    })?;

    let files = find_source_files(src_dir)?;
    for input_path in &files {
        check_project_file(input_path)?;
    }
    validate_project(src_dir)?;
    for input_path in &files {
        compile_project_file_unchecked(src_dir, out_dir, input_path)?;
    }

    Ok(files.len())
}

pub fn watch_project(src_dir: &Path, out_dir: &Path) -> Result<(), String> {
    let count = sync_project(src_dir, out_dir)?;
    println!(
        "OK: {} 件を {} から {} へ変換しました",
        count,
        display_path(src_dir),
        display_path(out_dir)
    );
    println!("watch: .rs の変更を監視しています。終了するには Ctrl+C を押してください。");

    let mut seen = snapshot_source_files(src_dir)?;
    let mut seen_outputs = snapshot_output_files(src_dir, out_dir, seen.keys())?;
    loop {
        thread::sleep(WATCH_INTERVAL);
        let current = snapshot_source_files(src_dir)?;
        let current_outputs = snapshot_output_files(src_dir, out_dir, current.keys())?;

        if current.keys().ne(seen.keys()) {
            write_manifest(src_dir)?;
        }

        for (input_path, stamp) in &current {
            let output_path = output_path_for(src_dir, out_dir, input_path)?;
            let source_changed = seen.get(input_path) != Some(stamp);
            let output_changed =
                seen_outputs.get(&output_path) != current_outputs.get(&output_path);
            if source_changed || output_changed {
                let output_path = compile_project_file(src_dir, out_dir, input_path)?;
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
) -> Result<PathBuf, String> {
    let output = compile_project_source(input_path)?;

    if is_rs_file(input_path) {
        validate_project(src_dir)?;
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

pub fn snapshot_source_files(src_dir: &Path) -> Result<BTreeMap<PathBuf, FileStamp>, String> {
    ensure_source_dir(src_dir)?;
    let mut snapshot = BTreeMap::new();

    for file in find_source_files(src_dir)? {
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
) -> Result<PathBuf, String> {
    let output = compile_project_source(input_path)?;
    write_project_output(src_dir, out_dir, input_path, output)
}

fn check_project_file(input_path: &Path) -> Result<(), String> {
    let source = read_project_source(input_path)?;
    transplanter::check_source(&source).map_err(|err| format_compile_error(input_path, err))
}

fn compile_project_source(input_path: &Path) -> Result<String, String> {
    let source = read_project_source(input_path)?;
    transplanter::compile_source(&source).map_err(|err| format_compile_error(input_path, err))
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

fn find_source_files(src_dir: &Path) -> Result<Vec<PathBuf>, String> {
    ensure_source_dir(src_dir)?;
    let mut files = Vec::new();
    collect_source_files(src_dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_source_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
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
            collect_source_files(&path, files)?;
        } else if metadata.is_file() && is_rs_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}
