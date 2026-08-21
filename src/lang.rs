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
        let path = context.display_path();
        match self {
            SpecFormat::CodeBlockPathExt => format_code_block(
                &path,
                context
                    .path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or(""),
                contents,
            ),
            SpecFormat::CodeBlock(lang) => format_code_block(&path, lang, contents),
        }
    }
}

fn format_code_block(path: &Path, lang: &str, contents: String) -> String {
    format!("### {}\n```{lang}\n{contents}\n```\n\n", path.display())
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
    pub input_roots: &'a [PathBuf],
}

impl<'a> LangContext<'a> {
    pub fn new(
        args: &'a clap::ArgMatches,
        path: &'a Path,
        excludes: &'a GlobSet,
        defaults_enabled: bool,
        input_roots: &'a [PathBuf],
    ) -> Self {
        let provided = excludes.is_match(path) || (defaults_enabled && has_dot_component(path));
        Self {
            args,
            path,
            visited: HashSet::new(),
            excludes,
            defaults_enabled,
            provided,
            input_roots,
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
            input_roots: self.input_roots,
        }
    }

    pub fn display_path(&self) -> PathBuf {
        display_path(
            self.path,
            self.input_roots,
            self.args.get_flag("relative-to-cwd"),
        )
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

pub fn normalize_input_roots(inputs: &[PathBuf]) -> Vec<PathBuf> {
    inputs.iter().map(|p| absolutize(p)).collect()
}

fn display_path(path: &Path, input_roots: &[PathBuf], relative_to_cwd: bool) -> PathBuf {
    let abs_path = absolutize(path);
    if relative_to_cwd {
        return strip_or_self(
            &abs_path,
            &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        );
    }

    let mut best: Option<&Path> = None;
    for root in input_roots {
        if is_under_or_equal(&abs_path, root) {
            if best.is_none_or(|b| component_count(root) > component_count(b)) {
                best = Some(root);
            }
        }
    }

    best.map(|root| strip_or_self(&abs_path, root))
        .unwrap_or_else(|| path.to_path_buf())
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .ok()
            .and_then(|p| fs::canonicalize(p).ok())
            .unwrap_or_else(|| path.to_path_buf())
    }
}

fn is_under_or_equal(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn component_count(path: &Path) -> usize {
    path.components().count()
}

fn strip_or_self(path: &Path, prefix: &Path) -> PathBuf {
    path.strip_prefix(prefix)
        .map(|rel| {
            if rel.as_os_str().is_empty() {
                path.file_name().map(PathBuf::from).unwrap_or_else(|| path.to_path_buf())
            } else {
                rel.to_path_buf()
            }
        })
        .unwrap_or_else(|_| path.to_path_buf())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("join-lang-test-{name}-{nanos}"))
    }

    #[test]
    fn display_path_uses_closest_input_parent() {
        let root = unique_tmp_dir("display-closest-parent");
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();
        fs::write(root.join("src/main.rs"), "main").unwrap();
        fs::write(root.join("src/nested/mod.rs"), "mod").unwrap();

        let inputs = normalize_input_roots(&[
            root.join("README.md"),
            root.join("src"),
        ]);

        assert_eq!(
            display_path(&root.join("src/main.rs"), &inputs, false),
            PathBuf::from("main.rs")
        );
        assert_eq!(
            display_path(&root.join("src/nested/mod.rs"), &inputs, false),
            PathBuf::from("nested/mod.rs")
        );
        assert_eq!(
            display_path(&root.join("README.md"), &inputs, false),
            PathBuf::from("README.md")
        );
    }

    #[test]
    fn display_path_prefers_deepest_matching_input() {
        let root = unique_tmp_dir("display-deepest-parent");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "main").unwrap();

        let inputs = normalize_input_roots(&[root.clone(), root.join("src")]);
        assert_eq!(
            display_path(&root.join("src/main.rs"), &inputs, false),
            PathBuf::from("main.rs")
        );
    }

    #[test]
    fn display_path_relative_to_cwd() {
        let root = unique_tmp_dir("display-cwd-relative");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "main").unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&root).unwrap();
        let inputs = normalize_input_roots(&[root.join("src")]);
        assert_eq!(
            display_path(&root.join("src/main.rs"), &inputs, true),
            PathBuf::from("src/main.rs")
        );
        std::env::set_current_dir(prev).unwrap();
    }
}
