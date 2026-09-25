use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fmt::Display};
use thiserror::Error;
use url::Url;

const CATALOG_JSON: &[u8] = include_bytes!("../../public/data/catalog.json");
const MANIFEST_JSON: &[u8] = include_bytes!("../../public/data/catalog.manifest.json");
const EXPECTED_SOURCE_SPECS: [(&str, &str, usize, &str); 3] = [
    (
        "characters",
        "キャラクターカタログ.md",
        128,
        "17f36462ddd545d68418e7414609d00e2bc26222c3f2fadf17063b0b21970787",
    ),
    (
        "weapons",
        "武器カタログ.md",
        252,
        "47959d88de440fda0ef52093988af124b750ce21faa12e6c2d3eb2409ffbb0ca",
    ),
    (
        "artifactSets",
        "聖遺物セットカタログ.md",
        63,
        "4b181b59aecd782abb506324fa2243db624d33139a56155528457b5f51e1693f",
    ),
];

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("カタログJSONを解析できません: {0}")]
    Json(#[from] serde_json::Error),
    #[error("カタログ検証に失敗しました: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub schema_version: String,
    pub game_version: String,
    pub catalog_updated_at: String,
    pub characters: Vec<Character>,
    pub weapons: Vec<Weapon>,
    pub artifact_sets: Vec<ArtifactSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub id: String,
    pub name: String,
    pub element: String,
    pub weapon_type: String,
    pub rarity: u8,
    pub image_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Weapon {
    pub id: String,
    pub name: String,
    pub weapon_type: String,
    pub rarity: u8,
    pub image_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSet {
    pub id: String,
    pub name: String,
    pub team_buff_key: Option<String>,
    pub two_piece_effect_group_id: String,
    pub two_piece_effect: String,
    pub four_piece_effect: Option<String>,
    pub piece_image_urls: PieceImageUrls,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PieceImageUrls {
    pub flower: String,
    pub plume: String,
    pub sands: String,
    pub goblet: String,
    pub circlet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogManifest {
    pub schema_version: String,
    pub game_version: String,
    pub catalog_updated_at: String,
    pub counts: CatalogCounts,
    pub catalog_sha256: String,
    pub sources: Vec<CatalogSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCounts {
    pub characters: usize,
    pub weapons: usize,
    pub artifact_sets: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSource {
    pub kind: String,
    pub file_name: String,
    pub declared_count: usize,
    pub sha256: String,
}

#[tauri::command]
pub fn load_catalog() -> Result<Catalog, String> {
    load_embedded_catalog().map_err(|error| error.to_string())
}

pub fn load_embedded_catalog() -> Result<Catalog, CatalogError> {
    let catalog: Catalog = serde_json::from_slice(CATALOG_JSON)?;
    let manifest: CatalogManifest = serde_json::from_slice(MANIFEST_JSON)?;
    validate_catalog(&catalog, Some(&manifest), CATALOG_JSON)?;
    Ok(catalog)
}

pub fn validate_catalog(
    catalog: &Catalog,
    manifest: Option<&CatalogManifest>,
    catalog_bytes: &[u8],
) -> Result<(), CatalogError> {
    if catalog.schema_version != "catalog-v2" {
        return Err(invalid("schemaVersionがcatalog-v2ではありません"));
    }
    if catalog.game_version.trim().is_empty() || catalog.catalog_updated_at.trim().is_empty() {
        return Err(invalid("ゲーム版または更新日が空です"));
    }
    validate_characters(&catalog.characters)?;
    validate_weapons(&catalog.weapons)?;
    validate_artifacts(&catalog.artifact_sets)?;

    if let Some(manifest) = manifest {
        if manifest.schema_version != "catalog-manifest-v2" {
            return Err(invalid("manifestのschemaVersionが不正です"));
        }
        if manifest.game_version != catalog.game_version
            || manifest.catalog_updated_at != catalog.catalog_updated_at
        {
            return Err(invalid("manifestのメタデータがカタログと一致しません"));
        }
        let counts = CatalogCounts {
            characters: catalog.characters.len(),
            weapons: catalog.weapons.len(),
            artifact_sets: catalog.artifact_sets.len(),
        };
        if manifest.counts != counts {
            return Err(invalid("manifestの件数がカタログと一致しません"));
        }
        let digest = hex_digest(catalog_bytes);
        if manifest.catalog_sha256 != digest {
            return Err(invalid("catalogSha256が一致しません"));
        }
        validate_manifest_sources(manifest, &counts)?;
    }
    Ok(())
}

fn validate_manifest_sources(
    manifest: &CatalogManifest,
    counts: &CatalogCounts,
) -> Result<(), CatalogError> {
    if manifest.sources.len() != EXPECTED_SOURCE_SPECS.len() {
        return Err(invalid("manifestのソース件数が不正です"));
    }
    let mut kinds = HashSet::new();
    for source in &manifest.sources {
        if !kinds.insert(source.kind.as_str()) {
            return Err(invalid(format!(
                "manifestのkindが重複しています: {}",
                source.kind
            )));
        }
        let expected = EXPECTED_SOURCE_SPECS
            .iter()
            .find(|(kind, _, _, _)| *kind == source.kind)
            .ok_or_else(|| invalid(format!("manifestのkindが不正です: {}", source.kind)))?;
        if source.file_name != expected.1 {
            return Err(invalid(format!(
                "manifestのfileNameが不正です: {}",
                source.file_name
            )));
        }
        let expected_count = match source.kind.as_str() {
            "characters" => counts.characters,
            "weapons" => counts.weapons,
            "artifactSets" => counts.artifact_sets,
            _ => unreachable!(),
        };
        if source.declared_count != expected_count || source.declared_count != expected.2 {
            return Err(invalid(format!(
                "manifestのdeclaredCountが不正です: {}",
                source.kind
            )));
        }
        if !is_lower_hex_sha256(&source.sha256) {
            return Err(invalid(format!(
                "manifestのsha256が不正です: {}",
                source.kind
            )));
        }
        if source.sha256 != expected.3 {
            return Err(invalid(format!(
                "manifestのソースSHA-256が不一致です: {}",
                source.kind
            )));
        }
    }
    if kinds.len() != EXPECTED_SOURCE_SPECS.len() {
        return Err(invalid("manifestのkindが不足しています"));
    }
    Ok(())
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_characters(characters: &[Character]) -> Result<(), CatalogError> {
    let mut ids = HashSet::new();
    for character in characters {
        if !ids.insert(&character.id) {
            return Err(invalid(format!(
                "キャラクターIDが重複しています: {}",
                character.id
            )));
        }
        if character.id.trim().is_empty() || character.name.trim().is_empty() {
            return Err(invalid("キャラクターのIDまたは名前が空です"));
        }
        if !["風", "岩", "雷", "草", "水", "炎", "氷", "無"].contains(&character.element.as_str())
        {
            return Err(invalid(format!("元素が不正です: {}", character.element)));
        }
        if !weapon_types().contains(&character.weapon_type.as_str()) {
            return Err(invalid(format!(
                "キャラクター武器種が不正です: {}",
                character.weapon_type
            )));
        }
        if ![4, 5].contains(&character.rarity) {
            return Err(invalid(format!(
                "キャラクターレアリティが不正です: {}",
                character.rarity
            )));
        }
        validate_image_url(&character.image_url)?;
    }
    Ok(())
}

fn validate_weapons(weapons: &[Weapon]) -> Result<(), CatalogError> {
    let mut ids = HashSet::new();
    for weapon in weapons {
        if !ids.insert(&weapon.id) {
            return Err(invalid(format!("武器IDが重複しています: {}", weapon.id)));
        }
        if weapon.id.trim().is_empty() || weapon.name.trim().is_empty() {
            return Err(invalid("武器のIDまたは名前が空です"));
        }
        if !weapon_types().contains(&weapon.weapon_type.as_str()) {
            return Err(invalid(format!("武器種が不正です: {}", weapon.weapon_type)));
        }
        if !(1..=5).contains(&weapon.rarity) {
            return Err(invalid(format!(
                "武器レアリティが不正です: {}",
                weapon.rarity
            )));
        }
        if let Ok(id) = weapon.id.parse::<u32>() {
            let expected = match id / 1000 {
                11 => "片手剣",
                12 => "両手剣",
                13 => "長柄武器",
                14 => "法器",
                15 => "弓",
                _ => "",
            };
            if expected.is_empty() || expected != weapon.weapon_type {
                return Err(invalid(format!(
                    "武器IDと武器種が不整合です: {}",
                    weapon.id
                )));
            }
        } else {
            return Err(invalid(format!(
                "武器IDが数値ではありません: {}",
                weapon.id
            )));
        }
        validate_image_url(&weapon.image_url)?;
    }
    Ok(())
}

fn validate_artifacts(artifacts: &[ArtifactSet]) -> Result<(), CatalogError> {
    let mut ids = HashSet::new();
    let mut effect_groups = std::collections::HashMap::new();
    for artifact in artifacts {
        if !ids.insert(&artifact.id) {
            return Err(invalid(format!(
                "聖遺物セットIDが重複しています: {}",
                artifact.id
            )));
        }
        if artifact.id.trim().is_empty()
            || artifact.name.trim().is_empty()
            || artifact.two_piece_effect_group_id.trim().is_empty()
            || artifact.two_piece_effect.trim().is_empty()
        {
            return Err(invalid("聖遺物セットの必須項目が空です"));
        }
        let expected_group = two_piece_effect_group_id(&artifact.two_piece_effect);
        if artifact.two_piece_effect_group_id != expected_group {
            return Err(invalid(format!(
                "2セット効果group IDが効果本文と一致しません: {}",
                artifact.id
            )));
        }
        if let Some(previous_effect) = effect_groups.insert(
            artifact.two_piece_effect_group_id.as_str(),
            artifact.two_piece_effect.as_str(),
        ) && previous_effect != artifact.two_piece_effect
        {
            return Err(invalid("異なる2セット効果が同じgroup IDを共有しています"));
        }
        if let Some(key) = &artifact.team_buff_key
            && key.trim().is_empty()
        {
            return Err(invalid("teamBuffKeyが空です"));
        }
        if let Some(effect) = &artifact.four_piece_effect
            && effect.trim().is_empty()
        {
            return Err(invalid("4セット効果が空です"));
        }
        validate_image_url(&artifact.piece_image_urls.flower)?;
        validate_image_url(&artifact.piece_image_urls.plume)?;
        validate_image_url(&artifact.piece_image_urls.sands)?;
        validate_image_url(&artifact.piece_image_urls.goblet)?;
        validate_image_url(&artifact.piece_image_urls.circlet)?;
    }
    Ok(())
}

pub fn two_piece_effect_group_id(effect: &str) -> String {
    let normalized = effect.trim().replace("\r\n", "\n").replace('\r', "\n");
    format!("two-piece-{}", hex_digest(normalized.as_bytes()))
}

pub fn validate_image_url(url: &str) -> Result<(), CatalogError> {
    let parsed = Url::parse(url).map_err(|_| invalid(format!("画像URLを解析できません: {url}")))?;
    let authority = url
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or_default();
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("gi.yatta.moe")
        || authority != "gi.yatta.moe"
        || !parsed.username().is_empty()
        || parsed.port().is_some()
        || parsed.path().is_empty()
        || parsed.path() == "/"
        || url.chars().any(char::is_whitespace)
    {
        return Err(invalid(format!("許可されていない画像URLです: {url}")));
    }
    Ok(())
}

fn weapon_types() -> [&'static str; 5] {
    ["片手剣", "両手剣", "長柄武器", "弓", "法器"]
}

pub fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn invalid(message: impl Into<String>) -> CatalogError {
    CatalogError::Invalid(message.into())
}

impl Display for CatalogCounts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}/{}/{}",
            self.characters, self.weapons, self.artifact_sets
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 埋込カタログの件数と旅人を検証できる() {
        let catalog = load_embedded_catalog().expect("埋込カタログが有効であること");
        assert_eq!(catalog.game_version, "7.1");
        assert_eq!(catalog.characters.len(), 128);
        assert_eq!(catalog.weapons.len(), 252);
        assert_eq!(catalog.artifact_sets.len(), 63);
        assert_eq!(
            catalog
                .characters
                .iter()
                .filter(|character| character.id.starts_with("traveler-"))
                .count(),
            7
        );
        assert_eq!(
            catalog
                .artifact_sets
                .iter()
                .filter(|artifact| artifact.four_piece_effect.is_none())
                .count(),
            4
        );
    }

    #[test]
    fn 改ざんされたchecksumを拒否する() {
        let catalog: Catalog = serde_json::from_slice(CATALOG_JSON).expect("解析できること");
        let manifest: CatalogManifest =
            serde_json::from_slice(MANIFEST_JSON).expect("解析できること");
        let result = validate_catalog(&catalog, Some(&manifest), b"tampered");
        assert!(result.is_err());
    }

    #[test]
    fn 最小カタログfixtureを検証できる() {
        let catalog = Catalog {
            schema_version: "catalog-v2".into(),
            game_version: "7.0".into(),
            catalog_updated_at: "2026-08-24".into(),
            characters: vec![Character {
                id: "traveler-anemo".into(),
                name: "旅人（風）".into(),
                element: "風".into(),
                weapon_type: "片手剣".into(),
                rarity: 5,
                image_url: "https://gi.yatta.moe/traveler.png".into(),
            }],
            weapons: vec![Weapon {
                id: "11101".into(),
                name: "無鋒の剣".into(),
                weapon_type: "片手剣".into(),
                rarity: 1,
                image_url: "https://gi.yatta.moe/sword.png".into(),
            }],
            artifact_sets: vec![ArtifactSet {
                id: "10001".into(),
                name: "旅人の心".into(),
                team_buff_key: None,
                two_piece_effect_group_id: two_piece_effect_group_id("攻撃力+18%。"),
                two_piece_effect: "攻撃力+18%。".into(),
                four_piece_effect: Some("重撃の会心率+30%。".into()),
                piece_image_urls: PieceImageUrls {
                    flower: "https://gi.yatta.moe/flower.png".into(),
                    plume: "https://gi.yatta.moe/plume.png".into(),
                    sands: "https://gi.yatta.moe/sands.png".into(),
                    goblet: "https://gi.yatta.moe/goblet.png".into(),
                    circlet: "https://gi.yatta.moe/circlet.png".into(),
                },
            }],
        };
        validate_catalog(&catalog, None, b"{}").expect("最小fixtureを検証できること");
    }

    #[test]
    fn validates_image_url_authority() {
        assert!(validate_image_url("https://gi.yatta.moe/assets/icon.png").is_ok());
        assert!(validate_image_url("https://example.com/assets/icon.png").is_err());
        assert!(validate_image_url("https://user@gi.yatta.moe/assets/icon.png").is_err());
        assert!(validate_image_url("https://gi.yatta.moe:443/assets/icon.png").is_err());
        assert!(validate_image_url("https://gi.yatta.moe").is_err());
    }

    #[test]
    fn manifestのソース改ざんを拒否する() {
        let catalog: Catalog = serde_json::from_slice(CATALOG_JSON).expect("解析できること");
        let mut manifest: CatalogManifest =
            serde_json::from_slice(MANIFEST_JSON).expect("解析できること");
        manifest.sources[0].sha256 = "0".repeat(64);
        assert!(validate_catalog(&catalog, Some(&manifest), CATALOG_JSON).is_err());
    }
}
