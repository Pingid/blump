use clap::{Arg, ArgAction, Command};
use globset::{GlobBuilder, GlobSetBuilder};
use std::{fs, process};
use std::{
    io::{self, BufRead, IsTerminal},
    path::PathBuf,
};

use crate::lang::{LangContext, specs};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = build_cli();

    let matches = cmd.clone().get_matches();

    // Collect inputs from args and stdin
    let inputs = collect_inputs(&matches)?;
    if inputs.is_empty() {
        cmd.print_long_help()?;
        process::exit(1);
    }

    // Handle default excludes
    let mut exclude = GlobSetBuilder::new();
    if !matches.get_flag("no-default-excludes") {
        for spec in specs() {
            for pattern in spec.exclude {
                exclude.add(
                    GlobBuilder::new(pattern)
                        .literal_separator(false)
                        .build()
                        .unwrap(),
                );
            }
        }
    }

    // Handle CLI-provided excludes
    if let Some(pats) = matches.get_many::<String>("exclude") {
        for pattern in pats {
            exclude.add(
                GlobBuilder::new(pattern)
                    .literal_separator(false)
                    .build()
                    .unwrap(),
            );
        }
    }
    let excludes = exclude.build().unwrap();
    let defaults_enabled = !matches.get_flag("no-default-excludes");
    for path in inputs {
        let mut context = LangContext::new(&matches, &path, &excludes, defaults_enabled);
        match handle_path(&mut context) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("join: Error reading {path:?}: {e}");
                process::exit(1);
            }
        }
    }

    Ok(())
}

fn build_cli() -> Command {
    let mut cmd = Command::new("join")
        .about("Concatenate files to markdown")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("exclude")
                .short('e')
                .long("exclude")
                .value_name("GLOB")
                .help("Exclude paths matching the glob (repeatable)")
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("no-default-excludes")
                .short('E')
                .long("no-default-excludes")
                .help("Disable built-in default excludes")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no-follow-symlinks")
                .long("no-follow-symlinks")
                .help("Do not follow symlinks (skip symlinked files and directories)")
                .action(ArgAction::SetTrue)
                .global(true),
        )
        .arg(
            Arg::new("inputs")
                .value_name("FILE")
                .help("Input files or directories")
                .num_args(0..),
        );

    for spec in specs() {
        for arg in &spec.args {
            cmd = cmd.arg(arg.clone());
        }
    }

    cmd
}

fn handle_path(context: &mut LangContext) -> Result<(), std::io::Error> {
    if context.excluded() || !context.visit() {
        return Ok(());
    }

    if context.args.get_flag("no-follow-symlinks") {
        let meta = fs::symlink_metadata(context.path)?;
        if meta.file_type().is_symlink() {
            return Ok(());
        }
    }

    let meta = fs::metadata(context.path)?;

    if meta.is_file() {
        let contents = fs::read_to_string(context.path);
        match contents {
            Err(e) => {
                eprintln!("Error {:?}: {e}", context.path);
                return Ok(());
            }
            Ok(contents) => {
                let spec = specs()
                    .into_iter()
                    .find(|spec| spec.matches.spec_matches(context))
                    .unwrap_or_default();
                let processed = spec.processor.process_contents(&context, contents);
                let formatted = spec.format.format_contents(&context, processed);
                print!("{}", formatted);
                return Ok(());
            }
        };
    }

    let entries = fs::read_dir(context.path)?;
    let mut files = Vec::new();
    let mut dirs = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if context.args.get_flag("no-follow-symlinks") {
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_file() {
                files.push(path);
            } else if meta.is_dir() {
                dirs.push(path);
            }
        } else {
            let meta = fs::metadata(&path)?;
            if meta.is_file() {
                files.push(path);
            } else if meta.is_dir() {
                dirs.push(path);
            }
        }
    }

    for spec in specs() {
        spec.sort.sort_files(&mut files)?;
    }

    // Process files first, then directories
    for path in files {
        handle_path(&mut context.child(&path))?;
    }
    for path in dirs {
        handle_path(&mut context.child(&path))?;
    }

    Ok(())
}

fn collect_inputs(matches: &clap::ArgMatches) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut inputs: Vec<PathBuf> = matches
        .get_many::<String>("inputs")
        .map(|vals| vals.map(PathBuf::from).collect())
        .unwrap_or_default();

    // Read from stdin if piped
    if !io::stdin().is_terminal() {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = line?;
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                inputs.push(PathBuf::from(trimmed));
            }
        }
    }

    Ok(inputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use globset::GlobSetBuilder;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    fn unique_tmp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("join-test-{name}-{nanos}"))
    }

    #[test]
    #[cfg(unix)]
    fn visit_dedupes_symlink_loops_by_default() {
        use std::os::unix::fs::symlink;

        let root = unique_tmp_dir("symlink-loop-default");
        fs::create_dir_all(&root).unwrap();
        symlink(&root, root.join("loop")).unwrap();

        let matches = build_cli()
            .try_get_matches_from(["join", root.to_str().unwrap()])
            .unwrap();
        let excludes = GlobSetBuilder::new().build().unwrap();
        let mut ctx = LangContext::new(&matches, &root, &excludes, true);
        assert!(ctx.visit());

        let loop_path = root.join("loop");
        let mut loop_ctx = ctx.child(&loop_path);
        assert!(!loop_ctx.visit(), "symlinked loop should be deduped");
    }

    #[test]
    #[cfg(unix)]
    fn visit_does_not_dedupe_symlink_paths_when_no_follow_symlinks() {
        use std::os::unix::fs::symlink;

        let root = unique_tmp_dir("symlink-loop-no-follow");
        fs::create_dir_all(&root).unwrap();
        symlink(&root, root.join("loop")).unwrap();

        let matches = build_cli()
            .try_get_matches_from(["join", "--no-follow-symlinks", root.to_str().unwrap()])
            .unwrap();
        let excludes = GlobSetBuilder::new().build().unwrap();
        let mut ctx = LangContext::new(&matches, &root, &excludes, true);
        assert!(ctx.visit());

        let loop_path = root.join("loop");
        let mut loop_ctx = ctx.child(&loop_path);
        assert!(
            loop_ctx.visit(),
            "symlink paths should not be deduped when not following"
        );
    }

    #[test]
    fn explicit_hidden_input_is_not_excluded() {
        let root = unique_tmp_dir("explicit-hidden-input");
        fs::create_dir_all(&root).unwrap();

        let hidden = root.join(".hidden");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("file.txt"), "ok").unwrap();

        let matches = build_cli()
            .try_get_matches_from(["join", hidden.to_str().unwrap()])
            .unwrap();
        let excludes = GlobSetBuilder::new().build().unwrap();

        let ctx = LangContext::new(&matches, &hidden, &excludes, true);
        assert!(!ctx.excluded(), "explicit input should not be excluded");

        let child = hidden.join("file.txt");
        let child_ctx = ctx.child(&child);
        assert!(
            !child_ctx.excluded(),
            "explicit input subtree should not be excluded"
        );
    }

    #[test]
    fn hidden_paths_are_excluded_when_not_explicitly_passed() {
        let root = unique_tmp_dir("implicit-hidden-path");
        fs::create_dir_all(&root).unwrap();

        let hidden = root.join(".hidden");
        fs::create_dir_all(&hidden).unwrap();

        let matches = build_cli()
            .try_get_matches_from(["join", root.to_str().unwrap()])
            .unwrap();
        let excludes = GlobSetBuilder::new().build().unwrap();

        let ctx = LangContext::new(&matches, &root, &excludes, true);
        assert!(!ctx.excluded());

        let hidden_ctx = ctx.child(&hidden);
        assert!(
            hidden_ctx.excluded(),
            "hidden paths should be excluded by default"
        );
    }
}
