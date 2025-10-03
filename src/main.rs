use std::env;

mod config;
mod runner;
mod tui;

const APP_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_COMMIT_HASH"), ")");

#[derive(Default, Debug, Clone)]
struct Options {
    tui: bool,
    config_file: Option<String>,
}

fn print_help() {
    println!(
        "ghop [options] <set-name>\n\nGhop reads commands from a YAML file (ghop.yml by default) and runs the named set.\n\nOptions:\n    -h, --help            Print this help message.\n    -v, --version         Print the version.\n    -t, --tui             Run in TUI mode.\n    -f, --file <FILE>     YAML file to load (default: ghop.yml).\n\nYAML format example (only supported format):\n    sets:\n      dev: [\"npm run dev\", \"cargo watch -x run\"]\n\nExamples:\n    ghop build\n    ghop -f ghop.yml dev\n"
    );
}

fn is_option(arg: &str) -> bool {
    arg.starts_with('-')
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();

    // Parse options
    let mut opts = Options::default();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !is_option(arg) {
            break;
        }
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-v" | "--version" => {
                println!("{}", APP_VERSION);
                return;
            }
            "-t" | "--tui" => {
                opts.tui = true;
                i += 1;
            }
            "-f" | "--file" => {
                if i + 1 >= args.len() {
                    eprintln!("-f/--file requires a file path");
                    std::process::exit(2);
                }
                opts.config_file = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                eprintln!("Unknown option: {arg}");
                print_help();
                std::process::exit(2);
            }
        }
    }

    // Determine commands from YAML (default ghop.yml) and require set name
    let cfg_path = opts.config_file.clone().unwrap_or_else(|| "ghop.yml".to_string());
    if i >= args.len() || is_option(&args[i]) {
        eprintln!("No set specified. Provide a set name to run (e.g., 'ghop build').");
        std::process::exit(1);
    }
    let set_name = args[i].clone();
    let commands = match config::load_commands_from_yaml(&cfg_path, &set_name) {
        Ok(cmds) => cmds,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if opts.tui {
        // Run async TUI mode (currently ignores per-command timeouts in TUI)
        let commands_str: Vec<String> = commands.iter().map(|c| c.command.clone()).collect();
        let rt = tokio::runtime::Builder::new_multi_thread().enable_io().enable_time().build().expect("tokio runtime");
        match rt.block_on(tui::run(commands_str)) {
            Ok(code) => {
                if code != 0 { std::process::exit(code); }
                return;
            }
            Err(e) => {
                eprintln!("TUI error: {e}");
                std::process::exit(1);
            }
        }
    }

    let code = runner::run_commands(commands);
    if code != 0 {
        std::process::exit(code);
    }
}
