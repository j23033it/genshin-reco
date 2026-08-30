use genshin_reco_lib::catalog::{
    ArtifactSet, Catalog, CatalogCounts, CatalogManifest, CatalogSource, Character, PieceImageUrls,
    Weapon, hex_digest, two_piece_effect_group_id, validate_catalog, validate_image_url,
};
use std::{env, fs, path::Path};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let check = args.last().map(String::as_str) == Some("--check");
    let argument_count = if check { 5 } else { 4 };
    if args.len() != argument_count {
        return Err(
            "使い方: import_catalog <characters.md> <weapons.md> <artifacts.md> <output_dir> [--check]".into(),
        );
    }
    let character_path = Path::new(&args[0]);
    let weapon_path = Path::new(&args[1]);
    let artifact_path = Path::new(&args[2]);
    let output_dir = Path::new(&args[3]);
    let characters_source =
        normalize_source(&fs::read_to_string(character_path).map_err(|e| e.to_string())?);
    let weapons_source =
        normalize_source(&fs::read_to_string(weapon_path).map_err(|e| e.to_string())?);
    let artifacts_source =
        normalize_source(&fs::read_to_string(artifact_path).map_err(|e| e.to_string())?);

    let character_metadata = parse_metadata(&characters_source)?;
    let weapon_metadata = parse_metadata(&weapons_source)?;
    let artifact_metadata = parse_metadata(&artifacts_source)?;
    for metadata in [&weapon_metadata, &artifact_metadata] {
        if metadata.game_version != character_metadata.game_version
            || metadata.updated_at != character_metadata.updated_at
        {
            return Err("3ファイルのゲーム版または更新日が一致しません".into());
        }
    }

    let characters = parse_characters(&characters_source)?;
    let weapons = parse_weapons(&weapons_source)?;
    let artifact_sets = parse_artifacts(&artifacts_source)?;
    let declared = [
        (
            "キャラクター",
            character_metadata.declared_count,
            characters.len(),
        ),
        ("武器", weapon_metadata.declared_count, weapons.len()),
        (
            "聖遺物",
            artifact_metadata.declared_count,
            artifact_sets.len(),
        ),
    ];
    for (kind, declared_count, actual_count) in declared {
        if declared_count != actual_count {
            return Err(format!(
                "{kind}の宣言件数と実件数が一致しません: {declared_count} != {actual_count}"
            ));
        }
    }
    let catalog = Catalog {
        schema_version: "catalog-v2".into(),
        game_version: character_metadata.game_version.clone(),
        catalog_updated_at: character_metadata.updated_at.clone(),
        characters,
        weapons,
        artifact_sets,
    };
    validate_catalog(&catalog, None, b"{}").map_err(|e| e.to_string())?;
    let catalog_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&catalog).map_err(|e| e.to_string())?
    );
    let catalog_bytes = catalog_json.as_bytes();
    let manifest = CatalogManifest {
        schema_version: "catalog-manifest-v2".into(),
        game_version: catalog.game_version.clone(),
        catalog_updated_at: catalog.catalog_updated_at.clone(),
        counts: CatalogCounts {
            characters: catalog.characters.len(),
            weapons: catalog.weapons.len(),
            artifact_sets: catalog.artifact_sets.len(),
        },
        catalog_sha256: hex_digest(catalog_bytes),
        sources: vec![
            source(
                "characters",
                "キャラクターカタログ.md",
                &characters_source,
                character_metadata.declared_count,
            )?,
            source(
                "weapons",
                "武器カタログ.md",
                &weapons_source,
                weapon_metadata.declared_count,
            )?,
            source(
                "artifactSets",
                "聖遺物セットカタログ.md",
                &artifacts_source,
                artifact_metadata.declared_count,
            )?,
        ],
    };
    let manifest_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?
    );
    let outputs = [
        (output_dir.join("catalog.json"), catalog_bytes.to_vec()),
        (
            output_dir.join("catalog.manifest.json"),
            manifest_json.into_bytes(),
        ),
        (
            output_dir.join("sources/キャラクターカタログ.md"),
            characters_source.into_bytes(),
        ),
        (
            output_dir.join("sources/武器カタログ.md"),
            weapons_source.into_bytes(),
        ),
        (
            output_dir.join("sources/聖遺物セットカタログ.md"),
            artifacts_source.into_bytes(),
        ),
    ];
    if check {
        check_outputs(&outputs)?;
    } else {
        fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
        fs::create_dir_all(output_dir.join("sources")).map_err(|e| e.to_string())?;
        for (path, bytes) in outputs {
            fs::write(path, bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn normalize_source(source: &str) -> String {
    source.replace("\r\n", "\n").replace('\r', "\n")
}

fn check_outputs(outputs: &[(std::path::PathBuf, Vec<u8>)]) -> Result<(), String> {
    for (path, expected) in outputs {
        let actual = fs::read(path)
            .map_err(|error| format!("check対象を読めません {}: {error}", path.display()))?;
        if &actual != expected {
            return Err(format!(
                "check対象が生成結果と一致しません: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

struct Metadata {
    game_version: String,
    updated_at: String,
    declared_count: usize,
}

fn parse_metadata(source: &str) -> Result<Metadata, String> {
    let value = |label: &str| {
        source
            .lines()
            .find_map(|line| line.strip_prefix(&format!("- {label}: ")).map(str::trim))
            .map(str::to_string)
            .ok_or_else(|| format!("メタデータが見つかりません: {label}"))
    };
    let game_version = value("ゲームバージョン")?;
    let updated_at = value("カタログ更新日")?;
    let declared_count = value("件数")?
        .parse::<usize>()
        .map_err(|_| "件数が数値ではありません".to_string())?;
    if game_version.is_empty() || updated_at.is_empty() {
        return Err("ゲーム版または更新日が空です".into());
    }
    Ok(Metadata {
        game_version,
        updated_at,
        declared_count,
    })
}

fn rows(source: &str, expected_columns: usize) -> Result<Vec<Vec<String>>, String> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with('|') || line.contains("---") {
                return None;
            }
            let row: Vec<String> = line
                .trim_matches('|')
                .split('|')
                .map(|cell| normalize(cell.trim()))
                .collect();
            (row.first().map(String::as_str) != Some("ID")).then_some(row)
        })
        .map(|row| {
            if row.len() != expected_columns {
                Err(format!(
                    "表の列数が不正です: expected={expected_columns}, actual={}",
                    row.len()
                ))
            } else {
                Ok(row)
            }
        })
        .collect()
}

fn normalize(value: &str) -> String {
    value.replace("\\n", "\n")
}

fn image(cell: &str) -> Result<String, String> {
    let body = cell
        .strip_prefix('!')
        .unwrap_or(cell)
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| format!("画像リンクが不正です: {cell}"))?;
    let separator = body
        .rfind("](")
        .ok_or_else(|| format!("画像リンクが不正です: {cell}"))?;
    let url = &body[separator + 2..];
    validate_image_url(url).map_err(|error| error.to_string())?;
    Ok(url.into())
}

fn parse_characters(source: &str) -> Result<Vec<Character>, String> {
    rows(source, 6)?
        .into_iter()
        .map(|row| {
            Ok(Character {
                id: row[0].clone(),
                name: row[1].clone(),
                element: row[2].clone(),
                weapon_type: row[3].clone(),
                rarity: row[4]
                    .parse()
                    .map_err(|_| format!("レアリティが不正です: {}", row[4]))?,
                image_url: image(&row[5])?,
            })
        })
        .collect()
}

fn parse_weapons(source: &str) -> Result<Vec<Weapon>, String> {
    rows(source, 5)?
        .into_iter()
        .map(|row| {
            Ok(Weapon {
                id: row[0].clone(),
                name: row[1].clone(),
                weapon_type: row[2].clone(),
                rarity: row[3]
                    .parse()
                    .map_err(|_| format!("レアリティが不正です: {}", row[3]))?,
                image_url: image(&row[4])?,
            })
        })
        .collect()
}

fn parse_artifacts(source: &str) -> Result<Vec<ArtifactSet>, String> {
    rows(source, 10)?
        .into_iter()
        .map(|row| {
            Ok(ArtifactSet {
                id: row[0].clone(),
                name: row[1].clone(),
                team_buff_key: nullable(&row[2]),
                two_piece_effect_group_id: two_piece_effect_group_id(&row[3]),
                two_piece_effect: row[3].clone(),
                four_piece_effect: nullable(&row[4]),
                piece_image_urls: PieceImageUrls {
                    flower: image(&row[5])?,
                    plume: image(&row[6])?,
                    sands: image(&row[7])?,
                    goblet: image(&row[8])?,
                    circlet: image(&row[9])?,
                },
            })
        })
        .collect()
}

fn nullable(value: &str) -> Option<String> {
    (value != "-").then(|| value.to_string())
}

fn source(
    kind: &str,
    file_name: &str,
    source: &str,
    declared_count: usize,
) -> Result<CatalogSource, String> {
    Ok(CatalogSource {
        kind: kind.into(),
        file_name: file_name.into(),
        declared_count,
        sha256: hex_digest(source.as_bytes()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_markdown_fixture() {
        let characters = "# キャラクターカタログ\n\n- ゲームバージョン: 7.0\n- カタログ更新日: 2026-08-24\n- 件数: 1\n\n| ID | 名前 | 元素 | 武器種 | レアリティ | 画像 |\n| --- | --- | --- | --- | ---: | --- |\n| traveler-anemo | 旅人（風） | 風 | 片手剣 | 5 | [画像](https://gi.yatta.moe/c.png) |\n";
        let weapons = "# 武器カタログ\n\n- ゲームバージョン: 7.0\n- カタログ更新日: 2026-08-24\n- 件数: 1\n\n| ID | 名前 | 武器種 | レアリティ | 画像 |\n| --- | --- | --- | ---: | --- |\n| 11101 | 無鋒の剣 | 片手剣 | 1 | [画像](https://gi.yatta.moe/w.png) |\n";
        let artifacts = "# 聖遺物セットカタログ\n\n- ゲームバージョン: 7.0\n- カタログ更新日: 2026-08-24\n- 件数: 1\n\n| ID | 名前 | チームバフキー | 2セット効果 | 4セット効果 | 花 | 羽 | 時計 | 杯 | 冠 |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n| 10001 | 旅人の心 | - | 攻撃力+18%。 | 一行目\\n・二行目 | [花](https://gi.yatta.moe/f.png) | [羽](https://gi.yatta.moe/p.png) | [時計](https://gi.yatta.moe/s.png) | [杯](https://gi.yatta.moe/g.png) | [冠](https://gi.yatta.moe/c.png) |\n";

        assert_eq!(
            parse_metadata(characters)
                .expect("メタデータを解析できること")
                .declared_count,
            1
        );
        assert_eq!(
            parse_characters(characters)
                .expect("キャラクターを解析できること")
                .len(),
            1
        );
        assert_eq!(
            parse_weapons(weapons).expect("武器を解析できること").len(),
            1
        );
        let artifact = parse_artifacts(artifacts).expect("聖遺物を解析できること");
        assert_eq!(artifact.len(), 1);
        assert_eq!(artifact[0].team_buff_key, None);
        assert_eq!(
            artifact[0].four_piece_effect.as_deref(),
            Some("一行目\n・二行目")
        );
        assert_eq!(normalize_source("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn 列数不正の表行を黙って破棄しない() {
        let malformed = "| ID | 名前 | 元素 | 武器種 | レアリティ | 画像 |\n| --- | --- | --- | --- | ---: | --- |\n| traveler-anemo | 旅人（風） | 風 | 片手剣 | 5 | 余分な|区切り |\n";
        assert!(parse_characters(malformed).is_err());
    }

    #[test]
    fn 括弧を含む画像urlを途中で切らない() {
        let url = "https://gi.yatta.moe/assets/a_(b).png";
        assert_eq!(image(&format!("![画像]({url})")).unwrap(), url);
        assert_eq!(image(&format!("[画像]({url})")).unwrap(), url);
        assert!(image("![画像](https://gi.yatta.moe/assets/a_(b).png)余分").is_err());
    }
}
