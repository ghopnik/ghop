# GHOP

Ghop (short for **Ghopnik**) is a tiny command-line helper that lets you launch **several commands in parallel**.

It supports two modes:

1. **Default (streamed)** — runs each command in a shell and streams their output to your terminal as lines arrive (interleaved).
2. **TUI (`--tui`)** — runs each command in a text-user interface where every command has its own panel.

---

## Table of contents

* [Installation](#installation)
* [Quick start](#quick-start)
* [Usage](#usage)
* [Examples](#examples)
* [How it works](#how-it-works)
* [TUI mode](#tui-mode)
* [Exit status & signals](#exit-status--signals)
* [Shells & quoting](#shells--quoting)
* [Tips & patterns](#tips--patterns)
* [Troubleshooting](#troubleshooting)
* [Contributing](#contributing)
* [License](#license)

---

## Installation

### From source (Rust toolchain)

You’ll need a recent Rust toolchain (stable) with `cargo`.

```bash
# Clone the repo
git clone https://github.com/<your-username>/ghop
cd ghop

# Install to your Cargo bin dir (~/.cargo/bin/ghop)
cargo install --path .

# Or build a local binary
cargo build --release
# then copy ./target/release/ghop somewhere on your PATH
```

### Prebuilt packages

> Coming soon:
>
> * Homebrew (`brew install ghop`)
> * Scoop/Chocolatey on Windows
> * Flatpak / other Linux package repos

If you publish packages, update this section with the exact commands.

---

## Quick start

Run two commands in parallel:

```bash
ghop "sleep 2 && echo A done" "sleep 1 && echo B done"
```

Run in the TUI so each command gets its own panel:

```bash
ghop --tui "cargo build" "cargo test" "cargo clippy"
```

---

## Usage

```text
ghop [options] <command1> <command2> ... <commandN>

Options:
  -h, --help       Print help and exit
  -v, --version    Print version and exit
  -t, --tui        Run in TUI mode (panels per command)
  -f, --file FILE  Load commands from YAML file; then specify the set name to run
```

* **Commands** are executed **via a shell** (see [Shells & quoting](#shells--quoting)).
* Output in default mode is **line-buffered** and printed as it arrives; lines from different commands may interleave.
* TUI mode groups output per command in separate panes.

---

## Examples

### Basic parallel runs

```bash
# Build + test in parallel
ghop "cargo build" "cargo test"

# Ping two hosts simultaneously
ghop "ping -c 3 1.1.1.1" "ping -c 3 8.8.8.8"

# Run a sequence in each command (use your shell's operators)
ghop "make -j4" "make test" "make lint"
```

### Use with scripts & Makefiles

```makefile
# Makefile
.PHONY: ci
ci:
	ghop "cargo fmt -- --check" \
	     "cargo clippy -- -D warnings" \
	     "cargo test --all"
```

### Windows (PowerShell)

```powershell
ghop 'ping 1.1.1.1 -n 3' 'ping 8.8.8.8 -n 3'
ghop 'npm run build' 'npm test'
```

> Note: quoting rules differ on Windows; see [Shells & quoting](#shells--quoting).

### YAML configuration (-f/--file)

You can store named sets of commands in a YAML file and run one set by name using the -f/--file flag.

Supported formats:

- Flat map of set names to arrays of commands:

```yaml
# ghop.yml
build: ["cargo build", "cargo test"]
lint:  ["cargo clippy", "cargo fmt -- --check"]
```

- Or with a top-level "sets" key:

```yaml
# ghop.yml
sets:
  dev:
    - npm run dev
    - cargo watch -x run
```

Usage:

```bash
# Run a set by name
ghop -f ghop.yml build

# TUI mode with a set
ghop --tui -f ghop.yml dev
```

---

## How it works

* **Process model:** Ghop spawns one child process per command via your platform’s default shell.
* **Streaming:** In default mode, stdout/stderr are read asynchronously and printed **line by line** to your terminal.
* **Isolation (TUI):** In `--tui` mode, each command’s output is routed to its own scrollable pane.

---

## TUI mode

Start with `--tui` (or `-t`). You’ll see one pane per command.

What to expect:

* A header listing running/finished commands.
* A panel per command showing the live log.
* Basic navigation (e.g., switching focus, scrolling) may be provided by on-screen hints if present in your build.

> If your terminal is small, panes will auto-resize; enlarge the window for a better view.

---

## Exit status & signals

* **Exit code:** `0` if **all** commands succeed. **Non-zero** if **any** command fails. (If multiple fail, Ghop returns a non-zero code.)
* **Ctrl-C:** Sends an interrupt to child processes. Depending on your shell/program, they may exit immediately or on the next safe point.

---

## Shells & quoting

Ghop runs commands **through a shell** so you can use operators like `&&`, `|`, `;`, redirection, env vars, etc.

* **Linux/macOS:** `/bin/sh -c "<your command>"` (or your default login shell, depending on implementation).
* **Windows:** Typically `powershell -NoProfile -Command "<your command>"` or `cmd.exe /C`, depending on how you’ve built/packaged Ghop.

**Quoting tips**

* Wrap each command in quotes if it contains spaces or shell metacharacters:

  ```bash
  ghop "echo 'Hello, world'" "cat data.txt | wc -l"
  ```
* On Windows PowerShell, prefer **single quotes** unless you need interpolation:

  ```powershell
  ghop 'echo Hello' 'npm run build && npm test'
  ```

> **Security note:** Because commands are sent to a shell, **never** pass untrusted input directly.

---

## Tips & patterns

* **Group related CI steps:** run format/lint/test together to shorten feedback loops.
* **Balance work:** mix quick and long-running jobs to keep panes informative.
* **Persistent logs:** if you need artifacts, direct output to files in your command:

  ```bash
  ghop "mytask >logs/task1.log 2>&1" "othertask >logs/task2.log 2>&1"
  ```
* **Colorful output:** most tools detect TTYs and keep colors; if yours doesn’t, pass flags like `--color=always`.

---

## Troubleshooting

* **“Command not found”**

    * Ensure the command exists in your **PATH** for the shell Ghop uses.
    * Try invoking via an explicit shell: `ghop "bash -lc 'mycmd --flag'"`.

* **Weird quoting/expansion**

    * Quote the *whole* command as one argument to Ghop.
    * On Windows, try PowerShell single quotes.

* **Interleaved lines are confusing**

    * Use `--tui` to visually separate command output into panels.
    * Or redirect each command to its own log file.

* **Long lines truncate in TUI**

    * Widen the terminal or pipe to files in default mode.

---

## Contributing

PRs and issues welcome! Helpful areas:

* Packaging (Homebrew/Chocolatey/Scoop/Flatpak)
* TUI ergonomics (navigation, filters, search)
* Quality-of-life flags (e.g., fail-fast, retries, timestamps, max parallelism)
* Cross-platform polishing

Dev loop:

```bash
# Run checks
cargo fmt --all
cargo clippy -- -D warnings
cargo test

# Try the binary
cargo run -- "echo hello" "echo world"
```

---

## License

MIT © A. Mochkin