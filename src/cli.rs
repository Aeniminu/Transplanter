use std::fs;
use std::path::PathBuf;

use crate::ide_support::write_manifest;
use crate::language::LanguageMode;
use crate::lisp_check::validate_lisp_file;
use crate::paths::{
    DEFAULT_OUT_DIR, DEFAULT_SRC_DIR, display_path, format_compile_error, is_lisp_file, is_rs_file,
};
use crate::project::{sync_project, watch_project};
use crate::rust_check::validate_single_file;

#[derive(Debug)]
struct Cli {
    input_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    show_help: bool,
    show_version: bool,
    check_only: bool,
    sync: bool,
    watch: bool,
    init_ide: bool,
    src_dir: PathBuf,
    out_dir: PathBuf,
    language: LanguageMode,
    language_set: bool,
    src_dir_set: bool,
    out_dir_set: bool,
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let cli = parse_cli(args)?;

    if cli.show_help {
        print_usage();
        return Ok(());
    }

    if cli.show_version {
        println!("transplanter {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if cli.init_ide {
        return run_init_ide_mode(&cli);
    }

    if cli.sync || cli.watch {
        return run_project_mode(&cli);
    }

    run_single_file_mode(&cli)
}

fn parse_cli(args: Vec<String>) -> Result<Cli, String> {
    if args.is_empty() {
        return Ok(Cli {
            input_path: None,
            output_path: None,
            show_help: true,
            show_version: false,
            check_only: false,
            sync: false,
            watch: false,
            init_ide: false,
            src_dir: PathBuf::from(DEFAULT_SRC_DIR),
            out_dir: PathBuf::from(DEFAULT_OUT_DIR),
            language: LanguageMode::Auto,
            language_set: false,
            src_dir_set: false,
            out_dir_set: false,
        });
    }

    let mut cli = Cli {
        input_path: None,
        output_path: None,
        show_help: false,
        show_version: false,
        check_only: false,
        sync: false,
        watch: false,
        init_ide: false,
        src_dir: PathBuf::from(DEFAULT_SRC_DIR),
        out_dir: PathBuf::from(DEFAULT_OUT_DIR),
        language: LanguageMode::Auto,
        language_set: false,
        src_dir_set: false,
        out_dir_set: false,
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                cli.show_help = true;
            }
            "-V" | "--version" => {
                cli.show_version = true;
            }
            "--check" => {
                cli.check_only = true;
            }
            "--sync" => {
                cli.sync = true;
            }
            "--watch" => {
                cli.watch = true;
            }
            "--init-ide" => {
                cli.init_ide = true;
            }
            "--src" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err("エラー: --src の後に入力フォルダが必要です".to_string());
                };
                cli.src_dir = PathBuf::from(path);
                cli.src_dir_set = true;
            }
            "--out" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err("エラー: --out の後に出力フォルダが必要です".to_string());
                };
                cli.out_dir = PathBuf::from(path);
                cli.out_dir_set = true;
            }
            "--language" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err(
                        "エラー: --language の後に auto、rust、lisp のどれかが必要です".to_string(),
                    );
                };
                cli.language = LanguageMode::parse(value).ok_or_else(|| {
                    format!("エラー: --language は auto、rust、lisp のどれかを指定してください: `{value}`")
                })?;
                cli.language_set = true;
            }
            "-o" | "--output" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err("エラー: -o/--output の後に出力パスが必要です".to_string());
                };
                cli.output_path = Some(PathBuf::from(path));
            }
            arg if arg.starts_with('-') => {
                return Err(format!("エラー: 不明なオプション `{arg}`"));
            }
            path => {
                if cli.input_path.is_some() {
                    return Err("エラー: 入力ファイルは1つだけ指定できます".to_string());
                }
                cli.input_path = Some(PathBuf::from(path));
            }
        }

        i += 1;
    }

    Ok(cli)
}

fn run_init_ide_mode(cli: &Cli) -> Result<(), String> {
    if cli.sync || cli.watch {
        return Err("エラー: --init-ide と --sync/--watch は同時に使えません".to_string());
    }

    if cli.input_path.is_some() {
        return Err("エラー: --init-ide と入力ファイルは同時に使えません".to_string());
    }

    if cli.output_path.is_some() {
        return Err("エラー: --init-ide と -o/--output は同時に使えません".to_string());
    }

    if cli.check_only {
        return Err("エラー: --init-ide と --check は同時に使えません".to_string());
    }

    if cli.out_dir_set {
        return Err("エラー: --init-ide では --out は使いません".to_string());
    }

    if cli.language_set {
        return Err("エラー: --language は --sync または --watch と一緒に使います".to_string());
    }

    fs::create_dir_all(&cli.src_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(&cli.src_dir)
        )
    })?;
    let manifest_path = write_manifest(&cli.src_dir)?;
    println!("OK: {} を生成しました", display_path(&manifest_path));
    Ok(())
}

fn run_project_mode(cli: &Cli) -> Result<(), String> {
    if cli.sync && cli.watch {
        return Err("エラー: --sync と --watch は同時に使えません".to_string());
    }

    if cli.input_path.is_some() {
        return Err("エラー: --sync/--watch と入力ファイルは同時に使えません".to_string());
    }

    if cli.output_path.is_some() {
        return Err("エラー: --sync/--watch と -o/--output は同時に使えません".to_string());
    }

    if cli.check_only {
        return Err("エラー: --sync/--watch と --check は同時に使えません".to_string());
    }

    if cli.watch {
        watch_project(&cli.src_dir, &cli.out_dir, cli.language)
    } else {
        let count = sync_project(&cli.src_dir, &cli.out_dir, cli.language)?;
        println!(
            "OK: {} 件を {} から {} へ変換しました",
            count,
            display_path(&cli.src_dir),
            display_path(&cli.out_dir)
        );
        Ok(())
    }
}

fn run_single_file_mode(cli: &Cli) -> Result<(), String> {
    if cli.src_dir_set || cli.out_dir_set {
        return Err("エラー: --src/--out は --sync または --watch と一緒に使います".to_string());
    }

    if cli.language_set {
        return Err("エラー: 単体ファイル変換では --language は使いません".to_string());
    }

    let Some(input_path) = &cli.input_path else {
        return Err("エラー: 入力ファイルが必要です".to_string());
    };

    if cli.check_only && cli.output_path.is_some() {
        return Err("エラー: --check と -o/--output は同時に使えません".to_string());
    }

    let source = fs::read_to_string(input_path).map_err(|err| {
        format!(
            "エラー: `{}` を読み込めません: {err}",
            display_path(input_path)
        )
    })?;

    if cli.check_only {
        check_single_source(input_path, &source)?;
        validate_source_file(input_path)?;
        println!("OK: {}", display_path(input_path));
        return Ok(());
    }

    let output = compile_single_source(input_path, &source)?;
    validate_source_file(input_path)?;

    if let Some(output_path) = &cli.output_path {
        fs::write(output_path, output).map_err(|err| {
            format!(
                "エラー: `{}` に書き込めません: {err}",
                display_path(output_path)
            )
        })?;
    } else {
        print!("{output}");
    }

    Ok(())
}

fn validate_source_file(input_path: &std::path::Path) -> Result<(), String> {
    validate_single_file(input_path)?;
    validate_lisp_file(input_path)
}

fn check_single_source(input_path: &std::path::Path, source: &str) -> Result<(), String> {
    if is_rs_file(input_path) {
        return transplanter::check_source(source)
            .map_err(|err| format_compile_error(input_path, err));
    }
    if is_lisp_file(input_path) {
        return transplanter::check_lisp_source(source)
            .map_err(|err| format_compile_error(input_path, err));
    }
    Err(format!(
        "エラー: `{}` は対応している入力ファイルではありません。`.rs`、`.scm`、`.lisp` を使ってください",
        display_path(input_path)
    ))
}

fn compile_single_source(input_path: &std::path::Path, source: &str) -> Result<String, String> {
    if is_rs_file(input_path) {
        return transplanter::compile_source(source)
            .map_err(|err| format_compile_error(input_path, err));
    }
    if is_lisp_file(input_path) {
        return transplanter::compile_lisp_source(source)
            .map_err(|err| format_compile_error(input_path, err));
    }
    Err(format!(
        "エラー: `{}` は対応している入力ファイルではありません。`.rs`、`.scm`、`.lisp` を使ってください",
        display_path(input_path)
    ))
}

fn print_usage() {
    println!(
        "Transplanter (耕訳機)\n\nUsage:\n  transplanter <input.rs|input.scm|input.lisp>\n  transplanter <input.rs|input.scm|input.lisp> -o <output.py>\n  transplanter <input.rs|input.scm|input.lisp> --check\n  transplanter --init-ide [--src play_src]\n  transplanter --sync [--src play_src] [--out py_src] [--language auto|rust|lisp]\n  transplanter --watch [--src play_src] [--out py_src] [--language auto|rust|lisp]\n  transplanter --version"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cli_empty_args_defaults_to_help() {
        let cli = parse_cli(Vec::new()).unwrap();
        assert!(cli.show_help);
    }

    #[test]
    fn toml_strings_escape_windows_paths() {
        assert_eq!(
            crate::paths::toml_string(r#"C:\Users\Player\The "Farm""#),
            r#""C:\\Users\\Player\\The \"Farm\"""#
        );
    }
}
