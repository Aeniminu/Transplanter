use std::env;
use std::process;

mod cli;
mod ide_support;
mod paths;
mod project;
mod rust_check;
mod rust_modules;

#[cfg(windows)]
mod updater;

#[cfg(windows)]
mod win_gui;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        #[cfg(windows)]
        {
            win_gui::detach_console();
            return win_gui::run();
        }
    }

    cli::run(args)
}
