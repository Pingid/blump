# join

Concatenate files to markdown with language-aware processing.

## Install

```bash
cargo install --git https://github.com/Pingid/join
```

## Usage

```bash
join [OPTIONS] <FILE> [FILE...]

# Pipe a list of paths (one per line)
printf "src/main.rs\nREADME.md\n" | join

# Options
  -e, --exclude <GLOB>       Exclude paths matching the glob (repeatable)
  -E, --no-default-excludes  Disable built-in default excludes
  --relative-to-cwd          Write paths relative to the current directory
  --rust-strip-tests         Strip test modules and functions from Rust files
  -V, --version              Print version
```

## Examples

```bash
# Basic usage
join ./src/*.rs > lib.md

# Exclude paths matching a glob (repeatable)
join --exclude 'target/*' .
join -e '*.log' -e 'node_modules/*' .

# Strip test code from Rust files
join --rust-strip-tests ./src/*.rs > production_code.md

# Disable built-in defaults
join --no-default-excludes .
join -E .
```

## Features

- **Language-aware processing**: Detects file types and applies appropriate formatting
- **Test stripping**: Remove test code from Rust source files with `--rust-strip-tests`
- **Smart file ordering**: Processes files in logical order (e.g., main.rs before other Rust files)
- **Flexible exclusion**: Glob patterns for excluding files and directories

## Behavior

- Accepts file and folder paths. Folders are traversed recursively.
- Files are processed before subdirectories within each directory.
- Piped input: When stdin is not a TTY, reads newline-separated paths alongside CLI arguments.
- Glob patterns: Support `*`, `?`, `[...]`, `**` matching against full paths.

## Default Excludes

Disable with `--no-default-excludes` or `-E`.

**Directories**: `target/`, `node_modules/`, `dist/`, `build/`, `out/`, `coverage/`, `vendor/`, `__pycache__/`

**Files**: Dotfiles (`.git/`, `.env`, etc.), lockfiles (`Cargo.lock`, `package-lock.json`, etc.), compiled artifacts (`*.pyc`, `*.pyo`, `*.pyd`), logs (`*.log`), OS files (`Thumbs.db`, `Desktop.ini`)

Add custom exclusions with `--exclude` or `-e`.
