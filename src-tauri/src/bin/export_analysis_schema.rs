use genshin_reco_lib::domain::character_research_output_schema;
use std::{env, fs, path::Path};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let check = args.iter().any(|arg| arg == "--check");
    let path = args
        .iter()
        .find(|arg| arg.as_str() != "--check")
        .map(String::as_str)
        .unwrap_or("schemas/character-research-output.schema.json");

    let mut expected = serde_json::to_string_pretty(&character_research_output_schema())
        .map_err(|error| format!("分析SchemaをJSON化できません: {error}"))?;
    expected.push('\n');

    if check {
        let actual = fs::read_to_string(path)
            .map_err(|error| format!("check対象の分析Schemaを読めません {path}: {error}"))?;
        if actual != expected {
            return Err(format!("分析Schemaが生成結果と一致しません: {path}"));
        }
        return Ok(());
    }

    let output = Path::new(path);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("分析Schemaの出力先を作成できません: {error}"))?;
    }
    fs::write(output, expected)
        .map_err(|error| format!("分析Schemaを書き込めません {path}: {error}"))
}
