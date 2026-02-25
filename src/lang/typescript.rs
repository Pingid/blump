use super::*;

pub fn spec() -> LangSpec {
    LangSpec {
        exclude: &[
            "**/node_modules",
            "**package-lock.json",
            "**yarn.lock",
            "**pnpm-lock.yaml",
            "**bun.lock",
        ],
        args: vec![],
        matches: SpecMatch::Ext(vec!["ts".to_string(), "tsx".to_string()]),
        sort: SpecSort::InOrder(vec![
            "README.md".to_string(),
            "index.ts".to_string(),
            "core.ts".to_string(),
            "lib.ts".to_string(),
            "types.ts".to_string(),
        ]),
        format: SpecFormat::CodeBlock("ts".to_string()),
        processor: SpecProcessor::Skip,
    }
}
