use std::env;
use std::fs;
use std::process;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_usage();
        return Ok(());
    }

    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err("error: missing path after -o/--output".to_string());
                };
                output_path = Some(path.clone());
            }
            arg if arg.starts_with('-') => {
                return Err(format!("error: unknown option `{arg}`"));
            }
            path => {
                if input_path.is_some() {
                    return Err("error: only one input file is supported".to_string());
                }
                input_path = Some(path.to_string());
            }
        }

        i += 1;
    }

    let Some(input_path) = input_path else {
        return Err("error: missing input file".to_string());
    };

    let source = fs::read_to_string(&input_path)
        .map_err(|err| format!("error: failed to read `{input_path}`: {err}"))?;
    let output = farmrs::compile_source(&source).map_err(|err| err.to_string())?;

    if let Some(output_path) = output_path {
        fs::write(&output_path, output)
            .map_err(|err| format!("error: failed to write `{output_path}`: {err}"))?;
    } else {
        print!("{output}");
    }

    Ok(())
}

fn print_usage() {
    println!("farmrs\n\nUsage:\n  farmrs <input.farmrs>\n  farmrs <input.farmrs> -o <output.py>");
}
