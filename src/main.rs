use std::env;

mod config;
mod runner;
mod tui;

#[derive(Default, Debug, Clone)]
struct Options {
    tui: bool,
    config_file: Option<String>,
}

fn print_help() {
    println!(
        "ghop [options] <command1> <command2> ... <commandN>\n\nOptions:\n    -h, --help          Print this help message.\n    -v, --version       Print the version.\n    -t, --tui           Run in TUI mode.\n    -f, --file <FILE>   Load commands from YAML file; then specify the set name to run.\n\nYAML format examples:\n    # Simple map of sets\n    build: [\"cargo build\", \"cargo test\"]\n    lint:  [\"cargo clippy\", \"cargo fmt -- --check\"]\n\n    # Or use a top-level 'sets' key\n    sets:\n      dev: [\"npm run dev\", \"cargo watch -x run\"]\n\nUsage with -f:\n    ghop -f ghop.yml build\n"
    );
}

fn is_option(arg: &str) -> bool {
    arg.starts_with('-')
}

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

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
                println!("{}", env!("CARGO_PKG_VERSION"));
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

    // Determine commands
    let commands: Vec<String>;
    if let Some(cfg_path) = opts.config_file.clone() {
        // The next non-option arg must be the set name
        if i >= args.len() || is_option(&args[i]) {
            eprintln!("When using -f/--file, you must specify the set name to run.");
            std::process::exit(1);
        }
        let set_name = args[i].clone();
        // Any extra trailing args are ignored for now
        commands = match config::load_commands_from_yaml(&cfg_path, &set_name) {
            Ok(cmds) => cmds,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        };
    } else {
        // Remaining args are commands
        commands = args.split_off(i);
        if commands.is_empty() {
            eprintln!("No commands provided. Use -h for help.");
            std::process::exit(1);
        }
    }

    if opts.tui {
        // Run async TUI mode
        let rt = tokio::runtime::Builder::new_multi_thread().enable_io().enable_time().build().expect("tokio runtime");
        match rt.block_on(tui::run(commands)) {
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
