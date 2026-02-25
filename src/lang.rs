use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use globset::GlobSet;

mod default;
mod rust;
mod typescript;

pub fn specs() -> Vec<LangSpec> {
    vec![rust::spec(), typescript::spec(), default::spec()]
}

#[derive(Debug, Default, Clone)]
pub struct LangSpec {
    /// Default patterns to exclude from processing.
    pub exclude: &'static [&'static str],
    /// Arguments to pass to the plugin's CLI.
    pub args: Vec<clap::Arg>,
    /// How to match paths to this plugin.
    pub matches: SpecMatch,
    /// How to sort files within a directory.
    pub sort: SpecSort,
    /// How to format the contents of a file.
    pub format: SpecFormat,
    /// How to process the contents of a file.
    pub processor: SpecProcessor,
}

#[derive(Debug, Default, Clone)]
pub enum SpecMatch {
    #[default]
    Match,
    Ext(Vec<String>),
}

impl SpecMatch {
    pub fn spec_matches(&self, context: &LangContext) -> bool {
        match self {
            SpecMatch::Match => true,
            SpecMatch::Ext(ext) => ext
                .iter()
                .any(|e| context.path.extension().and_then(|s| s.to_str()) == Some(e)),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum SpecSort {
    #[default]
    None,
    InOrder(Vec<String>),
}

impl SpecSort {
    pub fn sort_files(&self, files: &mut [PathBuf]) -> Result<(), std::io::Error> {
        match self {
            SpecSort::None => Ok(()),
            SpecSort::InOrder(order) => {
                files.sort_by(|a, b| {
                    let a_pos = order
                        .iter()
                        .position(|s| s == a.file_name().unwrap().to_str().unwrap())
                        .unwrap_or(usize::MAX);
                    let b_pos = order
                        .iter()
                        .position(|s| s == b.file_name().unwrap().to_str().unwrap())
                        .unwrap_or(usize::MAX);
                    a_pos.cmp(&b_pos)
                });
                Ok(())
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum SpecFormat {
    #[default]
    CodeBlockPathExt,
    CodeBlock(String),
}

impl SpecFormat {
    pub fn format_contents(&self, context: &LangContext, contents: String) -> String {
        match self {
            SpecFormat::CodeBlockPathExt => format_code_block(
                context.path,
                context
                    .path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or(""),
                contents,
            ),
            SpecFormat::CodeBlock(lang) => format_code_block(context.path, lang, contents),
        }
    }
}

fn format_code_block(path: &Path, lang: &str, contents: String) -> String {
    format!("### {path:?}\n```{lang}\n{contents}\n```\n\n")
}

#[derive(Debug, Default, Clone)]
pub enum SpecProcessor {
    #[default]
    Skip,
    Fn(fn(&LangContext, String) -> String),
}

impl SpecProcessor {
    pub fn process_contents(&self, context: &LangContext, contents: String) -> String {
        match self {
            SpecProcessor::Skip => contents,
            SpecProcessor::Fn(f) => f(context, contents),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LangContext<'a> {
    pub args: &'a clap::ArgMatches,
    pub path: &'a Path,
    pub excludes: &'a GlobSet,
    pub visited: HashSet<PathBuf>,
    pub defaults_enabled: bool,
    /// Indicates path was explicitly provided by the user.
    /// When true, exclude rules are not applied for this path.
    pub provided: bool,
}

impl<'a> LangContext<'a> {
    pub fn new(
        args: &'a clap::ArgMatches,
        path: &'a Path,
        excludes: &'a GlobSet,
        defaults_enabled: bool,
    ) -> Self {
        let provided = excludes.is_match(path) || (defaults_enabled && has_dot_component(path));
        Self {
            args,
            path,
            visited: HashSet::new(),
            excludes,
            defaults_enabled,
            provided,
        }
    }

    pub fn child(&self, path: &'a Path) -> Self {
        Self {
            args: self.args,
            path,
            visited: self.visited.clone(),
            excludes: self.excludes,
            defaults_enabled: self.defaults_enabled,
            provided: self.provided,
        }
    }

    pub fn visit(&mut self) -> bool {
        self.visited.insert(self.visit_key())
    }

    pub fn excluded(&self) -> bool {
        if self.provided {
            return false;
        }
        if self.excludes.is_match(self.path) {
            return true;
        }
        if self.defaults_enabled && self.has_dot_component() {
            return true;
        }
        false
    }

    fn visit_key(&self) -> PathBuf {
        if self.args.get_flag("no-follow-symlinks") {
            return self.path.to_path_buf();
        }

        // When following symlinks, de-dupe by canonical path to avoid infinite recursion from
        // symlinked directory loops (e.g. dir contains `loop -> dir`).
        fs::canonicalize(self.path).unwrap_or_else(|_| self.path.to_path_buf())
    }

    fn has_dot_component(&self) -> bool {
        has_dot_component(self.path)
    }
}

fn has_dot_component(path: &Path) -> bool {
    use std::ffi::OsStr;
    for comp in path.components() {
        if let std::path::Component::Normal(os) = comp {
            if let Some(s) = os.to_str() {
                if s.starts_with('.') {
                    return true;
                }
            } else if os == OsStr::new(".") {
                return true;
            }
        }
    }
    false
}
