use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ide_support::{remove_rust_ide_support, write_manifest};
use crate::language::LanguageMode;
use crate::paths::{
    DEFAULT_SRC_DIR, LEGACY_DEFAULT_SRC_DIR, SYSTEM_DIR, display_path, ensure_system_dir,
    should_skip_source_dir, toml_string,
};

pub(crate) const CONFIG_FILE_NAME: &str = "transplanter.toml";
pub(crate) const DEFAULT_MAIN_RS: &str = r#"use transplanter_rust::prelude::*;

fn main() {
    harvest();
}
"#;
pub(crate) const DEFAULT_MAIN_SCM: &str = r#"(use transplanter)

(define (main)
  (harvest))
"#;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Config {
    pub(crate) src_dir: String,
    pub(crate) out_dir: String,
    pub(crate) language: LanguageMode,
    pub(crate) last_release_tag: String,
    pub(crate) last_release_notes: String,
}

pub(crate) fn config_path() -> PathBuf {
    exe_dir().join(SYSTEM_DIR).join(CONFIG_FILE_NAME)
}

fn exe_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn load_or_create_initial_workspace(config_path: &Path) -> (Config, Option<String>) {
    let config_exists = config_path.is_file();
    let legacy_config_path = legacy_config_path_for(config_path);
    let legacy_config_exists = !config_exists && legacy_config_path.is_file();
    let mut config = if config_exists {
        match read_config(config_path) {
            Ok(config) => config,
            Err(err) => return (Config::default(), Some(err)),
        }
    } else if legacy_config_exists {
        match read_config(&legacy_config_path) {
            Ok(config) => config,
            Err(err) => return (Config::default(), Some(err)),
        }
    } else {
        default_initial_config(config_path)
    };
    let layout_changed = match check_workspace_layout(config_path, &mut config) {
        Ok(changed) => changed,
        Err(err) => return (config, Some(err)),
    };
    let config_needs_write = !config_exists || legacy_config_exists || layout_changed;

    match ensure_initial_workspace(config_path, &config, config_needs_write) {
        Ok(()) => {
            let cleanup_error = if legacy_config_exists {
                remove_legacy_config(&legacy_config_path).err()
            } else {
                None
            };
            (config, cleanup_error)
        }
        Err(err) => (config, Some(err)),
    }
}

pub(crate) fn prepare_existing_workspace(config: &Config) -> Result<(), String> {
    if config.src_dir.trim().is_empty() {
        return Ok(());
    }

    let src_dir = PathBuf::from(&config.src_dir);
    if !src_dir.is_dir() {
        return Ok(());
    }

    prepare_language_workspace(config)
}

pub(crate) fn prepare_language_workspace(config: &Config) -> Result<(), String> {
    ensure_starter_file(config)?;
    cleanup_generated_files_for_mode(config)?;

    if config.language.includes_rust() {
        write_manifest(&PathBuf::from(&config.src_dir))?;
    } else {
        remove_rust_ide_support(&PathBuf::from(&config.src_dir))?;
    }

    Ok(())
}

fn legacy_config_path_for(config_path: &Path) -> PathBuf {
    config_base_dir(config_path).join(CONFIG_FILE_NAME)
}

fn remove_legacy_config(path: &Path) -> Result<(), String> {
    fs::remove_file(path).map_err(|err| {
        format!(
            "エラー: 旧設定 `{}` を削除できません: {err}",
            display_path(path)
        )
    })
}

fn check_workspace_layout(config_path: &Path, config: &mut Config) -> Result<bool, String> {
    let mut changed = migrate_legacy_default_src_dir(config_path, config)?;
    cleanup_legacy_workspace_artifacts(config_path, config)?;
    if config.src_dir.trim().is_empty() {
        config.src_dir = default_src_dir_for_config(config_path)
            .to_string_lossy()
            .into_owned();
        changed = true;
    }
    Ok(changed)
}

fn migrate_legacy_default_src_dir(config_path: &Path, config: &mut Config) -> Result<bool, String> {
    let src_dir = config.src_dir.trim();
    if src_dir.is_empty() {
        return Ok(false);
    }

    let src_path = PathBuf::from(src_dir);
    if !is_legacy_default_src_path(config_path, &src_path) {
        return Ok(false);
    }

    let legacy_src_dir = absolute_config_relative_path(config_path, &src_path);
    let default_src_dir = default_src_dir_for_config(config_path);
    if legacy_src_dir == default_src_dir {
        return Ok(false);
    }

    migrate_source_dir_contents(&legacy_src_dir, &default_src_dir)?;
    config.src_dir = default_src_dir.to_string_lossy().into_owned();
    Ok(true)
}

fn migrate_source_dir_contents(from_dir: &Path, to_dir: &Path) -> Result<(), String> {
    if !from_dir.exists() {
        fs::create_dir_all(to_dir)
            .map_err(|err| format!("エラー: `{}` を作成できません: {err}", display_path(to_dir)))?;
        return Ok(());
    }

    if !from_dir.is_dir() {
        return Ok(());
    }

    if !to_dir.exists() {
        fs::rename(from_dir, to_dir).map_err(|err| {
            format!(
                "エラー: `{}` を `{}` へ移動できません: {err}",
                display_path(from_dir),
                display_path(to_dir)
            )
        })?;
        return Ok(());
    }

    fs::create_dir_all(to_dir)
        .map_err(|err| format!("エラー: `{}` を作成できません: {err}", display_path(to_dir)))?;

    let mut blocked = Vec::new();
    for entry in fs::read_dir(from_dir).map_err(|err| {
        format!(
            "エラー: `{}` を読み込めません: {err}",
            display_path(from_dir)
        )
    })? {
        let entry = entry.map_err(|err| {
            format!(
                "エラー: `{}` を読み込めません: {err}",
                display_path(from_dir)
            )
        })?;
        let path = entry.path();
        let target = to_dir.join(entry.file_name());
        if !target.exists() {
            fs::rename(&path, &target).map_err(|err| {
                format!(
                    "エラー: `{}` を `{}` へ移動できません: {err}",
                    display_path(&path),
                    display_path(&target)
                )
            })?;
            continue;
        }

        if same_file_contents(&path, &target) {
            remove_path(&path)?;
        } else {
            blocked.push(entry.file_name().to_string_lossy().into_owned());
        }
    }

    remove_empty_dir_or_report(from_dir)?;
    if blocked.is_empty() {
        return Ok(());
    }

    Err(format!(
        "エラー: `{}` から `{}` へ移行できない同名ファイルがあります: {}",
        display_path(from_dir),
        display_path(to_dir),
        blocked.join(", ")
    ))
}

fn same_file_contents(left: &Path, right: &Path) -> bool {
    let Ok(left_metadata) = fs::metadata(left) else {
        return false;
    };
    let Ok(right_metadata) = fs::metadata(right) else {
        return false;
    };
    if !left_metadata.is_file() || !right_metadata.is_file() {
        return false;
    }

    fs::read(left).is_ok_and(|left_contents| {
        fs::read(right).is_ok_and(|right_contents| left_contents == right_contents)
    })
}

fn remove_path(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|err| format!("エラー: `{}` を確認できません: {err}", display_path(path)))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|err| format!("エラー: `{}` を削除できません: {err}", display_path(path)))
}

fn remove_empty_dir_or_report(dir: &Path) -> Result<(), String> {
    if fs::read_dir(dir)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some()
    {
        return Ok(());
    }

    fs::remove_dir(dir)
        .map_err(|err| format!("エラー: `{}` を削除できません: {err}", display_path(dir)))
}

fn cleanup_legacy_workspace_artifacts(config_path: &Path, config: &Config) -> Result<(), String> {
    let base_dir = config_base_dir(config_path);
    let src_dir = if config.src_dir.trim().is_empty() {
        default_src_dir_for_config(config_path)
    } else {
        PathBuf::from(&config.src_dir)
    };

    if src_dir.parent() == Some(base_dir.as_path()) {
        remove_generated_file_if_exact(&base_dir.join("main.rs"), DEFAULT_MAIN_RS)?;
        remove_generated_file_if_exact(&base_dir.join("main.scm"), DEFAULT_MAIN_SCM)?;
        remove_generated_file_if_exact(&base_dir.join("main.lisp"), DEFAULT_MAIN_SCM)?;
    }

    Ok(())
}

fn is_legacy_default_src_path(config_path: &Path, src_path: &Path) -> bool {
    src_path == Path::new(LEGACY_DEFAULT_SRC_DIR)
        || absolute_config_relative_path(config_path, src_path)
            == config_base_dir(config_path).join(LEGACY_DEFAULT_SRC_DIR)
}

fn default_initial_config(config_path: &Path) -> Config {
    Config {
        src_dir: default_src_dir_for_config(config_path)
            .to_string_lossy()
            .into_owned(),
        out_dir: String::new(),
        language: LanguageMode::Rust,
        ..Config::default()
    }
}

fn default_src_dir_for_config(config_path: &Path) -> PathBuf {
    config_base_dir(config_path).join(DEFAULT_SRC_DIR)
}

fn absolute_config_relative_path(config_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        config_base_dir(config_path).join(path)
    }
}

fn config_base_dir(config_path: &Path) -> PathBuf {
    let parent = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if parent.file_name().and_then(|name| name.to_str()) == Some(SYSTEM_DIR) {
        parent.parent().map(Path::to_path_buf).unwrap_or(parent)
    } else {
        parent
    }
}

fn ensure_initial_workspace(
    config_path: &Path,
    config: &Config,
    write_current_config: bool,
) -> Result<(), String> {
    if write_current_config {
        write_config(config_path, config)?;
    }

    if config.src_dir.trim().is_empty() {
        return Ok(());
    }

    let src_dir = PathBuf::from(&config.src_dir);
    fs::create_dir_all(&src_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(&src_dir)
        )
    })?;

    prepare_language_workspace(config)
}

fn cleanup_generated_files_for_mode(config: &Config) -> Result<(), String> {
    let src_dir = PathBuf::from(&config.src_dir);
    match config.language {
        LanguageMode::Rust => {
            remove_generated_file_if_exact(&src_dir.join("main.scm"), DEFAULT_MAIN_SCM)
        }
        LanguageMode::Lisp => {
            remove_generated_file_if_exact(&src_dir.join("main.rs"), DEFAULT_MAIN_RS)
        }
        LanguageMode::Auto => Ok(()),
    }
}

fn remove_generated_file_if_exact(path: &Path, generated_contents: &str) -> Result<(), String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Ok(());
    };
    if contents != generated_contents {
        return Ok(());
    }

    fs::remove_file(path)
        .map_err(|err| format!("エラー: `{}` を削除できません: {err}", display_path(path)))
}

fn ensure_starter_file(config: &Config) -> Result<(), String> {
    if config.src_dir.trim().is_empty() {
        return Ok(());
    }

    let src_dir = PathBuf::from(&config.src_dir);
    let (language, file_name, contents) = match config.language {
        LanguageMode::Rust | LanguageMode::Auto => (LanguageMode::Rust, "main.rs", DEFAULT_MAIN_RS),
        LanguageMode::Lisp => (LanguageMode::Lisp, "main.scm", DEFAULT_MAIN_SCM),
    };

    if has_matching_source_file(&src_dir, language)? {
        return Ok(());
    }

    let main_path = src_dir.join(file_name);
    if !main_path.exists() {
        fs::write(&main_path, contents).map_err(|err| {
            format!(
                "エラー: `{}` に書き込めません: {err}",
                display_path(&main_path)
            )
        })?;
    }

    Ok(())
}

fn has_matching_source_file(dir: &Path, language: LanguageMode) -> Result<bool, String> {
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
            if has_matching_source_file(&path, language)? {
                return Ok(true);
            }
        } else if metadata.is_file() && language.accepts_path(&path) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn read_config(path: &Path) -> Result<Config, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("エラー: `{}` を読み込めません: {err}", display_path(path)))?;
    parse_config(&contents)
}

pub(crate) fn write_config(path: &Path, config: &Config) -> Result<(), String> {
    ensure_config_parent(path)?;
    let contents = render_config(config);
    fs::write(path, contents)
        .map_err(|err| format!("エラー: `{}` に書き込めません: {err}", display_path(path)))
}

fn ensure_config_parent(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    if parent.file_name().and_then(|name| name.to_str()) == Some(SYSTEM_DIR) {
        ensure_system_dir(parent)
    } else {
        fs::create_dir_all(parent)
            .map_err(|err| format!("エラー: `{}` を作成できません: {err}", display_path(parent)))
    }
}

pub(crate) fn render_config(config: &Config) -> String {
    format!(
        "src_dir = {}\nout_dir = {}\nlanguage = {}\nlast_release_tag = {}\nlast_release_notes = {}\n",
        toml_string(&config.src_dir),
        toml_string(&config.out_dir),
        toml_string(config.language.as_str()),
        toml_string(&config.last_release_tag),
        toml_string(&config.last_release_notes)
    )
}

pub(crate) fn parse_config(contents: &str) -> Result<Config, String> {
    let mut config = Config::default();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = parse_toml_string(value.trim())?;
        match key {
            "src_dir" => config.src_dir = value,
            "out_dir" => config.out_dir = value,
            "language" => {
                config.language = LanguageMode::parse(&value).ok_or_else(|| {
                    format!(
                        "エラー: language は auto、rust、lisp のどれかを指定してください: `{value}`"
                    )
                })?;
            }
            "last_release_tag" => config.last_release_tag = value,
            "last_release_notes" => config.last_release_notes = value,
            _ => {}
        }
    }
    Ok(config)
}

fn parse_toml_string(value: &str) -> Result<String, String> {
    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err("エラー: 設定ファイルの文字列は \"...\" で囲んでください".to_string());
    };

    let mut output = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            return Err("エラー: 設定ファイルの文字列エスケープが途中で終わっています".to_string());
        };
        match escaped {
            '\\' => output.push('\\'),
            '"' => output.push('"'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            other => output.push(other),
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn toml_string_escapes_windows_paths() {
        assert_eq!(
            toml_string(r#"C:\Users\Player\The "Farm""#),
            r#""C:\\Users\\Player\\The \"Farm\"""#
        );
    }

    #[test]
    fn config_round_trips_paths() {
        let config = Config {
            src_dir: r"C:\Users\Player\Desktop\farming\rs_src".to_string(),
            out_dir: r"C:\Users\Player\AppData\LocalLow\TheFarmerWasReplaced\Saves\Rust"
                .to_string(),
            language: LanguageMode::Lisp,
            last_release_tag: "v0.1.1".to_string(),
            last_release_notes: "更新内容".to_string(),
        };
        let rendered = render_config(&config);
        assert_eq!(parse_config(&rendered).unwrap(), config);
    }

    #[test]
    fn initial_setup_creates_project_files() {
        let workspace = temp_workspace("initial_setup");
        let config_path = system_config_path(&workspace);

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert_eq!(PathBuf::from(&config.src_dir), workspace.join("play_src"));
        assert_eq!(config.out_dir, "");
        assert_eq!(config.language, LanguageMode::Rust);
        assert!(config_path.is_file());
        assert!(workspace.join("play_src").join("main.rs").is_file());
        assert!(
            fs::read_to_string(workspace.join("play_src").join("main.rs"))
                .unwrap()
                .contains("harvest();")
        );
        assert!(!workspace.join("Cargo.toml").exists());
        assert!(workspace.join(".transplanter").join("Cargo.toml").is_file());
        assert!(!workspace.join("play_src").join("Cargo.toml").exists());
        assert!(
            workspace
                .join(".transplanter")
                .join("transplanter_rust")
                .join("src")
                .join("prelude.rs")
                .is_file()
        );
        assert!(!workspace.join(".transplanter_ide").exists());
        assert!(!workspace.join("play_src").join(".transplanter").exists());

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn config_without_language_defaults_to_auto() {
        let config = parse_config("src_dir = \"rs_src\"\nout_dir = \"py_src\"\n").unwrap();
        assert_eq!(config.language, LanguageMode::Auto);
    }

    #[test]
    fn existing_lisp_config_creates_lisp_starter() {
        let workspace = temp_workspace("initial_lisp_setup");
        let config_path = system_config_path(&workspace);
        let legacy_src_dir = workspace.join("rs_src");
        let src_dir = workspace.join("play_src");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&legacy_src_dir).unwrap();
        fs::write(
            &config_path,
            format!(
                "src_dir = {}\nout_dir = \"\"\nlanguage = \"lisp\"\n",
                toml_string(legacy_src_dir.to_string_lossy().as_ref())
            ),
        )
        .unwrap();

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert_eq!(config.language, LanguageMode::Lisp);
        assert_eq!(PathBuf::from(&config.src_dir), src_dir);
        assert!(src_dir.join("main.scm").is_file());
        assert!(!src_dir.join("main.rs").exists());
        assert!(!legacy_src_dir.exists());
        assert!(!workspace.join(".transplanter").join("Cargo.toml").exists());
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn missing_legacy_rs_src_config_uses_play_src() {
        let workspace = temp_workspace("missing_legacy_rs_src");
        let config_path = system_config_path(&workspace);
        let legacy_config_path = workspace.join("transplanter.toml");
        let legacy_src_dir = workspace.join("rs_src");
        fs::write(
            &legacy_config_path,
            format!(
                "src_dir = {}\nout_dir = \"\"\nlanguage = \"rust\"\n",
                toml_string(legacy_src_dir.to_string_lossy().as_ref())
            ),
        )
        .unwrap();

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert_eq!(PathBuf::from(&config.src_dir), workspace.join("play_src"));
        assert!(workspace.join("play_src").join("main.rs").is_file());
        assert!(!legacy_src_dir.exists());
        assert!(!legacy_config_path.exists());
        assert!(
            fs::read_to_string(config_path)
                .unwrap()
                .contains("play_src")
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn initial_setup_preserves_existing_main_rs() {
        let workspace = temp_workspace("initial_setup_preserve");
        let src_dir = workspace.join("play_src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("main.rs"),
            "fn main() {\n    quick_print(7);\n}\n",
        )
        .unwrap();

        let (_config, startup_error) =
            load_or_create_initial_workspace(&system_config_path(&workspace));

        assert_eq!(startup_error, None);
        assert_eq!(
            fs::read_to_string(src_dir.join("main.rs")).unwrap(),
            "fn main() {\n    quick_print(7);\n}\n"
        );

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn legacy_rs_src_contents_move_to_play_src_on_startup() {
        let workspace = temp_workspace("legacy_src_migration");
        let config_path = system_config_path(&workspace);
        let legacy_src_dir = workspace.join("rs_src");
        fs::create_dir_all(&legacy_src_dir).unwrap();
        fs::write(
            legacy_src_dir.join("main.rs"),
            "fn main() {\n    quick_print(7);\n}\n",
        )
        .unwrap();
        write_test_config(&config_path, &legacy_src_dir, LanguageMode::Rust);

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert_eq!(PathBuf::from(&config.src_dir), workspace.join("play_src"));
        assert!(!legacy_src_dir.exists());
        assert_eq!(
            fs::read_to_string(workspace.join("play_src").join("main.rs")).unwrap(),
            "fn main() {\n    quick_print(7);\n}\n"
        );
        assert!(
            fs::read_to_string(config_path)
                .unwrap()
                .contains("play_src")
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn legacy_rs_src_conflict_preserves_user_file() {
        let workspace = temp_workspace("legacy_src_conflict");
        let config_path = system_config_path(&workspace);
        let legacy_src_dir = workspace.join("rs_src");
        let play_src_dir = workspace.join("play_src");
        fs::create_dir_all(&legacy_src_dir).unwrap();
        fs::create_dir_all(&play_src_dir).unwrap();
        fs::write(
            legacy_src_dir.join("main.rs"),
            "fn main() {\n    harvest();\n}\n",
        )
        .unwrap();
        fs::write(
            play_src_dir.join("main.rs"),
            "fn main() {\n    quick_print(1);\n}\n",
        )
        .unwrap();
        write_test_config(&config_path, &legacy_src_dir, LanguageMode::Rust);

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert!(startup_error.is_some());
        assert_eq!(PathBuf::from(&config.src_dir), legacy_src_dir);
        assert_eq!(
            fs::read_to_string(legacy_src_dir.join("main.rs")).unwrap(),
            "fn main() {\n    harvest();\n}\n"
        );
        assert_eq!(
            fs::read_to_string(play_src_dir.join("main.rs")).unwrap(),
            "fn main() {\n    quick_print(1);\n}\n"
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn lisp_mode_removes_generated_rust_starter_and_support() {
        let workspace = temp_workspace("lisp_mode_cleanup");
        let config_path = system_config_path(&workspace);
        let src_dir = workspace.join("play_src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.rs"), DEFAULT_MAIN_RS).unwrap();
        write_manifest(&src_dir).unwrap();
        write_test_config(&config_path, &src_dir, LanguageMode::Lisp);

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert_eq!(config.language, LanguageMode::Lisp);
        assert!(src_dir.join("main.scm").is_file());
        assert!(!src_dir.join("main.rs").exists());
        assert!(!workspace.join(".transplanter").join("Cargo.toml").exists());
        assert!(
            !workspace
                .join(".transplanter")
                .join("transplanter_rust")
                .exists()
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn rust_mode_removes_only_generated_lisp_starter() {
        let workspace = temp_workspace("rust_mode_cleanup");
        let config_path = system_config_path(&workspace);
        let src_dir = workspace.join("play_src");
        let edited_lisp = src_dir.join("edited.scm");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.scm"), DEFAULT_MAIN_SCM).unwrap();
        fs::write(&edited_lisp, format!("{DEFAULT_MAIN_SCM}\n; user note\n")).unwrap();
        write_test_config(&config_path, &src_dir, LanguageMode::Rust);

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert_eq!(config.language, LanguageMode::Rust);
        assert!(src_dir.join("main.rs").is_file());
        assert!(!src_dir.join("main.scm").exists());
        assert!(edited_lisp.is_file());
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn auto_mode_preserves_mixed_generated_starters() {
        let workspace = temp_workspace("auto_mode_preserve");
        let config_path = system_config_path(&workspace);
        let src_dir = workspace.join("play_src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.scm"), DEFAULT_MAIN_SCM).unwrap();
        write_test_config(&config_path, &src_dir, LanguageMode::Auto);

        let (config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert_eq!(config.language, LanguageMode::Auto);
        assert!(src_dir.join("main.rs").is_file());
        assert!(src_dir.join("main.scm").is_file());
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn startup_removes_legacy_generated_rust_artifacts() {
        let workspace = temp_workspace("legacy_generated_artifacts");
        let config_path = system_config_path(&workspace);
        let src_dir = workspace.join("play_src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.rs"), DEFAULT_MAIN_RS).unwrap();
        fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname = \"transplanter-scripts\"\nautobins = false\n\n[dependencies]\ntransplanter_rust = { path = \".transplanter_ide/transplanter_rust\" }\n",
        )
        .unwrap();
        fs::write(
            workspace.join("Cargo.lock"),
            "[[package]]\nname = \"transplanter-scripts\"\n\n[[package]]\nname = \"transplanter_rust\"\n",
        )
        .unwrap();
        crate::ide_support::write_support_crate(&workspace.join(".transplanter_ide")).unwrap();
        write_test_config(&config_path, &src_dir, LanguageMode::Rust);

        let (_config, startup_error) = load_or_create_initial_workspace(&config_path);

        assert_eq!(startup_error, None);
        assert!(!workspace.join("Cargo.toml").exists());
        assert!(!workspace.join("Cargo.lock").exists());
        assert!(!workspace.join(".transplanter_ide").exists());
        assert!(workspace.join(".transplanter").join("Cargo.toml").is_file());
        let _ = fs::remove_dir_all(workspace);
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "transplanter_workspace_setup_{name}_{}_{}",
            std::process::id(),
            suffix
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn system_config_path(workspace: &Path) -> PathBuf {
        workspace.join(".transplanter").join("transplanter.toml")
    }

    fn write_test_config(config_path: &Path, src_dir: &Path, language: LanguageMode) {
        let config = Config {
            src_dir: src_dir.to_string_lossy().into_owned(),
            out_dir: String::new(),
            language,
            ..Config::default()
        };
        write_config(config_path, &config).unwrap();
    }
}
