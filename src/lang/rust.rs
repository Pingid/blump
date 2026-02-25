use super::*;

mod strip_tests;

pub fn spec() -> LangSpec {
    LangSpec {
        exclude: &["**/target", "**Cargo.lock"],
        args: vec![
            clap::Arg::new("rust-strip-tests")
                .long("rust-strip-tests")
                .help("Strip test modules and functions from Rust files")
                .action(clap::ArgAction::SetTrue)
                .global(true),
        ],
        matches: SpecMatch::Ext(vec!["rs".to_string()]),
        sort: SpecSort::InOrder(vec![
            "main.rs".to_string(),
            "lib.rs".to_string(),
            "mod.rs".to_string(),
        ]),
        format: SpecFormat::CodeBlock("rust".to_string()),
        processor: SpecProcessor::Fn(process),
    }
}

fn process(context: &LangContext, contents: String) -> String {
    if context.args.get_flag("rust-strip-tests") {
        strip_tests::strip(&contents)
    } else {
        contents
    }
}
