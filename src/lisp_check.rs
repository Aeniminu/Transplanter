use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::{display_path, is_lisp_file};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SchemeChecker {
    GuileGuild,
    GuileCommand,
    ChezScheme(&'static str),
}

impl SchemeChecker {
    fn command(self) -> &'static str {
        match self {
            Self::GuileGuild => "guild",
            Self::GuileCommand => "guile",
            Self::ChezScheme(command) => command,
        }
    }

    fn command_path(self) -> OsString {
        match self {
            Self::GuileGuild => env::var_os("TRANSPLANTER_GUILE_GUILD")
                .unwrap_or_else(|| OsString::from(self.command())),
            Self::GuileCommand => {
                env::var_os("TRANSPLANTER_GUILE").unwrap_or_else(|| OsString::from(self.command()))
            }
            Self::ChezScheme("chezscheme") => env::var_os("TRANSPLANTER_CHEZ_SCHEME")
                .unwrap_or_else(|| OsString::from(self.command())),
            Self::ChezScheme(_) => OsString::from(self.command()),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::GuileGuild => "Guile Scheme (`guild compile`)",
            Self::GuileCommand => "Guile Scheme (`guile -s`)",
            Self::ChezScheme("chezscheme") => "Chez Scheme (`chezscheme --script`)",
            Self::ChezScheme(_) => "Chez Scheme (`scheme --script`)",
        }
    }
}

pub fn validate_lisp_file(input_path: &Path) -> Result<(), String> {
    if !is_lisp_file(input_path) {
        return Ok(());
    }

    let source = fs::read_to_string(input_path).map_err(|err| {
        format!(
            "エラー: Lisp検査対象 `{}` を読み込めません: {err}",
            display_path(input_path)
        )
    })?;
    validate_lisp_source(input_path, &source)
}

pub fn validate_lisp_source(input_path: &Path, source: &str) -> Result<(), String> {
    validate_lisp_source_with_checkers(
        input_path,
        source,
        &[
            SchemeChecker::GuileGuild,
            SchemeChecker::GuileCommand,
            SchemeChecker::ChezScheme("chezscheme"),
            SchemeChecker::ChezScheme("scheme"),
        ],
    )
}

fn validate_lisp_source_with_checkers(
    input_path: &Path,
    source: &str,
    checkers: &[SchemeChecker],
) -> Result<(), String> {
    let temp_dir = lisp_validation_temp_dir();
    fs::create_dir_all(&temp_dir).map_err(|err| {
        format!(
            "エラー: Lisp検査用フォルダ `{}` を作成できません: {err}",
            display_path(&temp_dir)
        )
    })?;

    let result = (|| {
        let validation_path = temp_dir.join("transplanter_lisp_check.scm");
        fs::write(&validation_path, validation_source(source)).map_err(|err| {
            format!(
                "エラー: Lisp検査用ファイル `{}` を作成できません: {err}",
                display_path(&validation_path)
            )
        })?;

        let mut not_found = Vec::new();
        for checker in checkers {
            match run_checker(*checker, &temp_dir, &validation_path) {
                Ok(output) if output.status.success() => return Ok(()),
                Ok(output) => {
                    return Err(format!(
                        "エラー: `{}` は{}で検査した結果、Schemeとしてコンパイルできません。\n{}",
                        display_path(input_path),
                        checker.name(),
                        command_details(&output)
                    ));
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    not_found.push(checker.command());
                }
                Err(err) => {
                    return Err(format!("エラー: {}を起動できません: {err}", checker.name()));
                }
            }
        }

        Err(format!(
            "エラー: `{}` はLisp入力ですが、外部Scheme検査に使う Guile Scheme (`guild` / `guile`) または Chez Scheme (`chezscheme` / `scheme`) が見つかりません。\nGuile または Chez Scheme をインストールしてPATHに追加してください。\n試したコマンド: {}",
            display_path(input_path),
            not_found.join(", ")
        ))
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    result
}

fn run_checker(
    checker: SchemeChecker,
    temp_dir: &Path,
    validation_path: &Path,
) -> io::Result<Output> {
    match checker {
        SchemeChecker::GuileGuild => Command::new(checker.command_path())
            .arg("compile")
            .arg("-o")
            .arg(temp_dir.join("transplanter_lisp_check.go"))
            .arg(validation_path)
            .current_dir(temp_dir)
            .output(),
        SchemeChecker::GuileCommand => Command::new(checker.command_path())
            .arg("--no-auto-compile")
            .arg("-s")
            .arg(validation_path)
            .current_dir(temp_dir)
            .output(),
        SchemeChecker::ChezScheme(_) => {
            let driver_path = temp_dir.join("transplanter_lisp_check_driver.scm");
            fs::write(
                &driver_path,
                format!("(compile-file {})\n", scheme_string(validation_path)),
            )?;
            Command::new(checker.command_path())
                .arg("--script")
                .arg(&driver_path)
                .current_dir(temp_dir)
                .output()
        }
    }
}

fn validation_source(source: &str) -> String {
    format!("{}\n\n{}", validation_prelude(), source)
}

fn validation_prelude() -> &'static str {
    r#"; Generated by Transplanter for external Scheme validation.
(define (use . args) #t)

(define-syntax loop
  (syntax-rules ()
    ((_ body ...)
     (let transplanter-loop ()
       body ...
       (transplanter-loop)))))

(define-syntax for
  (syntax-rules ()
    ((_ var start end body ...)
     (let transplanter-for ((var start))
       (if (< var end)
           (begin
             body ...
             (transplanter-for (+ var 1)))
           #t)))))

(define (transplanter-any . args) #t)
(define :north 'north)
(define :east 'east)
(define :south 'south)
(define :west 'west)

(define harvest transplanter-any)
(define can-harvest transplanter-any)
(define swap transplanter-any)
(define plant transplanter-any)
(define move transplanter-any)
(define move-dir transplanter-any)
(define till transplanter-any)
(define trade transplanter-any)
(define get-pos-x transplanter-any)
(define get-pos-y transplanter-any)
(define get-world-size transplanter-any)
(define get-entity-type transplanter-any)
(define get-ground-type transplanter-any)
(define get-tick-count transplanter-any)
(define get-time transplanter-any)
(define get-op-count transplanter-any)
(define use-item transplanter-any)
(define get-water transplanter-any)
(define do-a-flip transplanter-any)
(define print transplanter-any)
(define quick-print transplanter-any)
(define len transplanter-any)
(define num-items transplanter-any)
(define get-cost transplanter-any)
(define clear transplanter-any)
(define get-companion transplanter-any)
(define unlock transplanter-any)
(define num-unlocked transplanter-any)
(define timed-reset transplanter-any)
(define measure transplanter-any)
(define dict transplanter-any)
(define set transplanter-any)
(define set-execution-speed transplanter-any)
(define set-farm-size transplanter-any)
(define simulate transplanter-any)
(define leaderboard-run transplanter-any)
(define direction transplanter-any)
(define entity transplanter-any)
(define ground transplanter-any)
(define item transplanter-any)
(define leaderboard transplanter-any)
(define index transplanter-any)
(define set-index! transplanter-any)
"#
}

fn scheme_string(path: &Path) -> String {
    let mut output = String::from("\"");
    for ch in path.to_string_lossy().chars() {
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

fn command_details(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        stderr.trim().to_string()
    }
}

fn lisp_validation_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!(
        "transplanter_lisp_check_{}_{}",
        process::id(),
        nanos
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_string_escapes_windows_paths() {
        assert_eq!(
            scheme_string(Path::new(r#"C:\Users\Player\my "farm".scm"#)),
            r#""C:\\Users\\Player\\my \"farm\".scm""#
        );
    }

    #[test]
    fn reports_missing_checker() {
        let err = validate_lisp_source_with_checkers(
            Path::new("main.scm"),
            "(define (main) (harvest))",
            &[],
        )
        .unwrap_err();
        assert!(err.contains("外部Scheme検査"), "{err}");
    }
}
