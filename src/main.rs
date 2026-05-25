use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::{Duration, SystemTime};

const DEFAULT_SRC_DIR: &str = "rs_src";
const DEFAULT_OUT_DIR: &str = "py_src";
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = parse_cli(env::args().skip(1).collect())?;

    if cli.show_help {
        print_usage();
        return Ok(());
    }

    if cli.show_version {
        println!("farmrs {}", env!("CARGO_PKG_VERSION"));
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
    src_dir_set: bool,
    out_dir_set: bool,
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

    fs::create_dir_all(&cli.src_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(&cli.src_dir)
        )
    })?;
    let manifest_path = write_ide_manifest(&cli.src_dir)?;
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
        watch_project(&cli.src_dir, &cli.out_dir)
    } else {
        let count = sync_project(&cli.src_dir, &cli.out_dir)?;
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
        farmrs::check_source(&source).map_err(|err| format_compile_error(input_path, err))?;
        println!("OK: {}", display_path(input_path));
        return Ok(());
    }

    let output =
        farmrs::compile_source(&source).map_err(|err| format_compile_error(input_path, err))?;

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

fn sync_project(src_dir: &Path, out_dir: &Path) -> Result<usize, String> {
    ensure_source_dir(src_dir)?;
    write_ide_manifest(src_dir)?;
    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "エラー: `{}` を作成できません: {err}",
            display_path(out_dir)
        )
    })?;

    let files = find_source_files(src_dir)?;
    for input_path in &files {
        compile_project_file(src_dir, out_dir, input_path)?;
    }

    Ok(files.len())
}

fn watch_project(src_dir: &Path, out_dir: &Path) -> Result<(), String> {
    let count = sync_project(src_dir, out_dir)?;
    println!(
        "OK: {} 件を {} から {} へ変換しました",
        count,
        display_path(src_dir),
        display_path(out_dir)
    );
    println!("watch: .rs / .farmrs の変更を監視しています。終了するには Ctrl+C を押してください。");

    let mut seen = snapshot_source_files(src_dir)?;
    loop {
        thread::sleep(WATCH_INTERVAL);
        let current = snapshot_source_files(src_dir)?;

        if current.keys().ne(seen.keys()) {
            write_ide_manifest(src_dir)?;
        }

        for (input_path, modified) in &current {
            if seen.get(input_path) != Some(modified) {
                compile_project_file(src_dir, out_dir, input_path)?;
                println!("OK: {} を変換しました", display_path(input_path));
            }
        }

        seen = current;
    }
}

fn compile_project_file(src_dir: &Path, out_dir: &Path, input_path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(input_path).map_err(|err| {
        format!(
            "エラー: `{}` を読み込めません: {err}",
            display_path(input_path)
        )
    })?;
    let output =
        farmrs::compile_source(&source).map_err(|err| format_compile_error(input_path, err))?;
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
    })
}

fn output_path_for(src_dir: &Path, out_dir: &Path, input_path: &Path) -> Result<PathBuf, String> {
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

fn snapshot_source_files(src_dir: &Path) -> Result<BTreeMap<PathBuf, SystemTime>, String> {
    ensure_source_dir(src_dir)?;
    let mut snapshot = BTreeMap::new();

    for file in find_source_files(src_dir)? {
        let modified = fs::metadata(&file)
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("エラー: `{}` を確認できません: {err}", display_path(&file)))?;
        snapshot.insert(file, modified);
    }

    Ok(snapshot)
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

        if metadata.is_dir() {
            collect_source_files(&path, files)?;
        } else if metadata.is_file() && is_source_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn write_ide_manifest(src_dir: &Path) -> Result<PathBuf, String> {
    let manifest_path = src_dir.join("Cargo.toml");
    let rs_files = find_rs_files(src_dir)?;
    let manifest = render_ide_manifest(src_dir, &rs_files)?;
    fs::write(&manifest_path, manifest).map_err(|err| {
        format!(
            "エラー: `{}` に書き込めません: {err}",
            display_path(&manifest_path)
        )
    })?;
    Ok(manifest_path)
}

fn render_ide_manifest(src_dir: &Path, rs_files: &[PathBuf]) -> Result<String, String> {
    let mut manifest = String::new();
    manifest.push_str("[package]\n");
    manifest.push_str("name = \"farmrs-scripts\"\n");
    manifest.push_str("version = \"0.1.0\"\n");
    manifest.push_str("edition = \"2024\"\n");
    manifest.push_str("publish = false\n");
    manifest.push_str("autobins = false\n\n");
    manifest.push_str("[dependencies]\n");
    manifest.push_str(&format!(
        "farmrs = {{ path = {} }}\n",
        toml_string(env!("CARGO_MANIFEST_DIR"))
    ));

    let mut used_names = BTreeSet::new();
    for input_path in rs_files {
        let relative = input_path.strip_prefix(src_dir).map_err(|_| {
            format!(
                "エラー: `{}` は `{}` の中にありません",
                display_path(input_path),
                display_path(src_dir)
            )
        })?;
        let name = unique_bin_name(relative, &mut used_names);
        manifest.push_str("\n[[bin]]\n");
        manifest.push_str(&format!("name = {}\n", toml_string(&name)));
        manifest.push_str(&format!(
            "path = {}\n",
            toml_string(&relative_path_for_manifest(relative))
        ));
    }

    Ok(manifest)
}

fn find_rs_files(src_dir: &Path) -> Result<Vec<PathBuf>, String> {
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

        if metadata.is_dir() {
            collect_rs_files(&path, files)?;
        } else if metadata.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }

    Ok(())
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

fn relative_path_for_manifest(relative: &Path) -> String {
    relative.to_string_lossy().replace('\\', "/")
}

fn toml_string(value: &str) -> String {
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

fn is_source_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext == "rs" || ext == "farmrs")
}

fn ensure_source_dir(src_dir: &Path) -> Result<(), String> {
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

fn format_compile_error(path: &Path, err: farmrs::error::FarmError) -> String {
    format!(
        "エラー: {}:{}行{}列: {}",
        display_path(path),
        err.line,
        err.column,
        err.message
    )
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn print_usage() {
    println!(
        "farmrs\n\nUsage:\n  farmrs <input.rs|input.farmrs>\n  farmrs <input.rs|input.farmrs> -o <output.py>\n  farmrs <input.rs|input.farmrs> --check\n  farmrs --init-ide [--src rs_src]\n  farmrs --sync [--src rs_src] [--out py_src]\n  farmrs --watch [--src rs_src] [--out py_src]\n  farmrs --version"
    );
}
