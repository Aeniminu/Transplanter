use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::{
    IDE_SUPPORT_CRATE_DIR, display_path, ensure_source_dir, is_rs_file, project_dir_for_src_dir,
    relative_path_for_manifest, should_skip_source_dir, toml_string,
};
use crate::rust_modules::discover_module_files;

pub fn write_manifest(src_dir: &Path) -> Result<PathBuf, String> {
    let rs_files = find_rs_files(src_dir)?;
    write_manifest_for_files(src_dir, &rs_files)
}

pub fn write_manifest_for_files(src_dir: &Path, rs_files: &[PathBuf]) -> Result<PathBuf, String> {
    let project_dir = project_dir_for_src_dir(src_dir);
    let manifest_path = project_dir.join("Cargo.toml");
    write_support_crate(&project_dir)?;
    let module_files = discover_module_files(rs_files)?;
    let manifest = render_manifest(&project_dir, src_dir, rs_files, &module_files)?;
    fs::write(&manifest_path, manifest).map_err(|err| {
        format!(
            "エラー: `{}` に書き込めません: {err}",
            display_path(&manifest_path)
        )
    })?;
    Ok(manifest_path)
}

pub fn write_support_crate(project_dir: &Path) -> Result<(), String> {
    let crate_dir = project_dir.join(IDE_SUPPORT_CRATE_DIR);
    let src_support_dir = crate_dir.join("src");
    fs::create_dir_all(&src_support_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(&src_support_dir)
        )
    })?;

    let manifest_path = crate_dir.join("Cargo.toml");
    fs::write(&manifest_path, support_manifest()).map_err(|err| {
        format!(
            "エラー: `{}` に書き込めません: {err}",
            display_path(&manifest_path)
        )
    })?;

    let lib_path = src_support_dir.join("lib.rs");
    fs::write(&lib_path, "pub mod prelude;\n").map_err(|err| {
        format!(
            "エラー: `{}` に書き込めません: {err}",
            display_path(&lib_path)
        )
    })?;

    let prelude_path = src_support_dir.join("prelude.rs");
    fs::write(
        &prelude_path,
        include_str!("../converters/rust_to_python/src/prelude.rs"),
    )
    .map_err(|err| {
        format!(
            "エラー: `{}` に書き込めません: {err}",
            display_path(&prelude_path)
        )
    })?;

    Ok(())
}

pub fn find_rs_files(src_dir: &Path) -> Result<Vec<PathBuf>, String> {
    ensure_source_dir(src_dir)?;
    let mut files = Vec::new();
    collect_rs_files(src_dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
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
            collect_rs_files(&path, files)?;
        } else if metadata.is_file() && is_rs_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn render_manifest(
    project_dir: &Path,
    src_dir: &Path,
    rs_files: &[PathBuf],
    module_files: &BTreeSet<PathBuf>,
) -> Result<String, String> {
    let mut manifest = String::new();
    manifest.push_str("[package]\n");
    manifest.push_str("name = \"transplanter-scripts\"\n");
    manifest.push_str("version = \"0.1.0\"\n");
    manifest.push_str("edition = \"2024\"\n");
    manifest.push_str("publish = false\n");
    manifest.push_str("autobins = false\n\n");
    manifest.push_str("[dependencies]\n");
    manifest.push_str(&format!(
        "transplanter_rust = {{ path = {} }}\n",
        toml_string(IDE_SUPPORT_CRATE_DIR)
    ));

    let mut used_names = BTreeSet::new();
    for input_path in rs_files {
        if module_files.contains(input_path) {
            continue;
        }
        let source_relative = input_path.strip_prefix(src_dir).map_err(|_| {
            format!(
                "エラー: `{}` は `{}` の中にありません",
                display_path(input_path),
                display_path(src_dir)
            )
        })?;
        let manifest_relative = relative_to_manifest(project_dir, input_path)?;
        let name = unique_bin_name(source_relative, &mut used_names);
        manifest.push_str("\n[[bin]]\n");
        manifest.push_str(&format!("name = {}\n", toml_string(&name)));
        manifest.push_str(&format!(
            "path = {}\n",
            toml_string(&relative_path_for_manifest(&manifest_relative))
        ));
    }

    Ok(manifest)
}

fn relative_to_manifest(project_dir: &Path, input_path: &Path) -> Result<PathBuf, String> {
    if project_dir == Path::new(".") {
        return Ok(input_path.to_path_buf());
    }

    input_path
        .strip_prefix(project_dir)
        .map(Path::to_path_buf)
        .map_err(|_| {
            format!(
                "エラー: `{}` は `{}` の中にありません",
                display_path(input_path),
                display_path(project_dir)
            )
        })
}

fn unique_bin_name(relative: &Path, used_names: &mut BTreeSet<String>) -> String {
    let mut base = relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "_");
    base = base
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if base.is_empty() || base.starts_with(|ch: char| ch.is_ascii_digit()) {
        base = format!("script_{base}");
    }

    let mut candidate = base.clone();
    let mut suffix = 2;
    while !used_names.insert(candidate.clone()) {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    candidate
}

fn support_manifest() -> &'static str {
    "[package]\nname = \"transplanter_rust\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\npath = \"src/lib.rs\"\n"
}
