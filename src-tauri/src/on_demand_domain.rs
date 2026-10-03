use crate::{
    catalog::Catalog,
    domain::make_strict_output_schema,
    game::GameId,
    star_rail::{RelicInput, StarRailBuild},
};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResearchConversationStatus {
    Collecting,
    Ready,
    Researching,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResearchMessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchMessage {
    pub role: ResearchMessageRole,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchMemberInput {
    #[serde(default)]
    pub relics: Option<RelicInput>,
    #[schemars(range(min = 0, max = 3))]
    pub slot_index: u8,
    pub name: String,
    pub weapon: Option<String>,
    #[schemars(range(min = 0, max = 6))]
    pub constellation: Option<u8>,
    #[schemars(range(min = 1, max = 5))]
    pub refinement: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchIntake {
    #[serde(default)]
    pub game: GameId,
    #[schemars(length(max = 4))]
    pub members: Vec<ResearchMemberInput>,
    #[schemars(length(max = 16))]
    pub missing_fields: Vec<String>,
    pub ready_to_research: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchConversation {
    #[serde(default)]
    pub game: GameId,
    pub session_id: String,
    pub status: ResearchConversationStatus,
    pub messages: Vec<ResearchMessage>,
    pub members: Vec<ResearchMemberInput>,
    #[serde(default)]
    pub title: Option<String>,
    pub missing_fields: Vec<String>,
    pub team_id: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntakeAgentOutput {
    pub assistant_message: String,
    pub intake: ResearchIntake,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchedTargetStat {
    pub label: String,
    pub value: String,
    pub primary: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchSource {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchedTeamMember {
    // Older saved results did not carry a structured Genshin refinement rank.
    #[serde(default)]
    #[schemars(range(min = 1, max = 5))]
    pub refinement: Option<u8>,
    #[serde(default)]
    pub star_rail: Option<StarRailBuild>,
    pub slot_index: u8,
    pub id: String,
    pub name: String,
    pub element: String,
    pub role: String,
    pub constellation: String,
    pub image_url: Option<String>,
    pub weapon: String,
    pub weapon_image_url: Option<String>,
    pub artifact: String,
    pub artifact_image_url: Option<String>,
    pub main_stats: String,
    pub sub_stats: String,
    #[schemars(length(min = 2, max = 8))]
    pub target_stats: Vec<ResearchedTargetStat>,
}

impl ResearchedTeamMember {
    pub(crate) fn apply_catalog_images(&mut self, catalog: &Catalog) {
        if self.star_rail.is_some() {
            if let Ok(catalog) = crate::star_rail::load_star_rail_catalog() {
                crate::star_rail::apply_images(self, &catalog);
            }
            return;
        }
        let name = normalize_asset_name(&self.name);
        self.image_url = catalog
            .characters
            .iter()
            .find(|character| {
                normalize_asset_name(&character.name) == name
                    || (name == "旅人"
                        && character.id.starts_with("traveler-")
                        && character.element == self.element.trim().trim_end_matches("元素"))
            })
            .map(|character| character.image_url.clone());

        let weapon = normalize_asset_name(&self.weapon);
        self.weapon_image_url = catalog
            .weapons
            .iter()
            .find(|candidate| {
                matches_asset_name(
                    &weapon,
                    &candidate.name,
                    &[
                        "R1", "R2", "R3", "R4", "R5", "精錬1", "精錬2", "精錬3", "精錬4", "精錬5",
                    ],
                )
            })
            .map(|weapon| weapon.image_url.clone());

        let artifact = normalize_asset_name(&self.artifact);
        self.artifact_image_url = catalog
            .artifact_sets
            .iter()
            .find(|candidate| {
                matches_asset_name(
                    &artifact,
                    &candidate.name,
                    &["4セット", "4セット効果", "4件", "4"],
                )
            })
            .map(|artifact| artifact.piece_image_urls.flower.clone());
    }
}

fn normalize_asset_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .map(|character| match character {
            '（' => '(',
            '）' => ')',
            '１' => '1',
            '２' => '2',
            '３' => '3',
            '４' => '4',
            '５' => '5',
            _ => character,
        })
        .collect()
}

fn matches_asset_name(label: &str, name: &str, suffixes: &[&str]) -> bool {
    let name = normalize_asset_name(name);
    let Some(suffix) = label.strip_prefix(&name) else {
        return false;
    };
    // 複数候補や2+2セットを、先頭の名前だけで単一の画像に決めない。
    suffix.is_empty()
        || suffixes.iter().any(|allowed| {
            suffix == *allowed
                || suffix
                    .strip_prefix('(')
                    .and_then(|value| value.strip_suffix(')'))
                    == Some(*allowed)
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchedTeamDraft {
    #[serde(default)]
    pub team_reasoning: Option<String>,
    #[serde(default)]
    pub game: GameId,
    pub title: String,
    pub game_version: String,
    #[schemars(length(min = 4, max = 4))]
    pub members: Vec<ResearchedTeamMember>,
    #[schemars(length(min = 1, max = 32))]
    pub sources: Vec<ResearchSource>,
    #[schemars(length(max = 16))]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchedTeamRecord {
    #[serde(default)]
    pub input_members: Option<Vec<ResearchMemberInput>>,
    #[serde(default)]
    pub team_reasoning: Option<String>,
    #[serde(default)]
    pub game: GameId,
    pub team_id: String,
    pub session_id: String,
    pub title: String,
    pub game_version: String,
    pub members: Vec<ResearchedTeamMember>,
    pub sources: Vec<ResearchSource>,
    pub warnings: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchedTeamSummary {
    #[serde(default)]
    pub game: GameId,
    pub team_id: String,
    pub title: String,
    pub member_names: Vec<String>,
    pub member_image_urls: Vec<Option<String>>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OnDemandResearchProgress {
    #[serde(default)]
    pub game: GameId,
    pub session_id: String,
    pub stage: String,
    pub detail: String,
    pub member_name: Option<String>,
}

pub fn validated_team_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 80 {
        return Err("編成名は1〜80文字で入力してください".into());
    }
    Ok(title.to_owned())
}

pub fn intake_output_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schema_for!(IntakeAgentOutput))
        .expect("受付出力SchemaをJSONへ変換できる");
    make_strict_output_schema(&mut schema);
    schema
}

pub fn team_research_output_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schema_for!(ResearchedTeamDraft))
        .expect("編成調査SchemaをJSONへ変換できる");
    make_strict_output_schema(&mut schema);
    schema
}

pub fn intake_output_schema_for(game: GameId) -> serde_json::Value {
    let mut schema = intake_output_schema();
    schema["$defs"]["ResearchIntake"]["properties"]["game"] =
        serde_json::json!({"type": "string", "enum": [game.key()]});
    if game == GameId::Genshin {
        schema["$defs"]["ResearchMemberInput"]["properties"]["relics"] =
            serde_json::json!({"type": "null"});
    }
    schema
}

pub fn team_research_output_schema_for(game: GameId) -> serde_json::Value {
    let mut schema = team_research_output_schema();
    schema["properties"]["game"] = serde_json::json!({"type": "string", "enum": [game.key()]});
    schema["$defs"]["ResearchedTeamMember"]["properties"]["starRail"] = if game == GameId::Genshin {
        serde_json::json!({"type": "null"})
    } else {
        serde_json::json!({"$ref": "#/$defs/StarRailBuild"})
    };
    schema["$defs"]["ResearchedTeamMember"]["properties"]["refinement"] = if game == GameId::Genshin
    {
        serde_json::json!({"type": "integer", "minimum": 1, "maximum": 5})
    } else {
        serde_json::json!({"type": "null"})
    };
    schema
}

/// Bind identity and fixed equipment to this request, while leaving build advice open.
pub fn team_research_output_schema_for_intake(
    intake: &ResearchIntake,
) -> Result<serde_json::Value, String> {
    intake.validate()?;
    if intake.members.len() != 4 {
        return Err("調査には4人の指定が必要です".into());
    }
    let mut schema = team_research_output_schema_for(intake.game);
    let catalog = crate::catalog::load_embedded_catalog().map_err(|error| error.to_string())?;
    let mut choices = Vec::new();
    for input in &intake.members {
        let mut member = schema["$defs"]["ResearchedTeamMember"].clone();
        let properties = &mut member["properties"];
        properties["slotIndex"] =
            serde_json::json!({"type": "integer", "enum": [input.slot_index]});
        properties["name"] = serde_json::json!({"type": "string", "enum": [input.name.trim()]});
        if intake.game == GameId::Genshin {
            if let Some(character) = catalog
                .characters
                .iter()
                .find(|c| c.name == input.name.trim())
            {
                properties["id"] = serde_json::json!({"type": "string", "enum": [character.id]});
            }
            if let Some(rank) = input.refinement {
                properties["refinement"] = serde_json::json!({"type": "integer", "enum": [rank]});
            }
        } else {
            let mut build = schema["$defs"]["StarRailBuild"].clone();
            if let Some(level) = input.constellation {
                build["properties"]["eidolon"] =
                    serde_json::json!({"type": "integer", "enum": [level]});
            }
            if let Some(weapon) = &input.weapon {
                build["properties"]["lightCone"] =
                    serde_json::json!({"type": "string", "enum": [weapon]});
            }
            if let Some(rank) = input.refinement {
                build["properties"]["superimposition"] =
                    serde_json::json!({"type": "integer", "enum": [rank]});
            }
            properties["starRail"] = build;
        }
        if let Some(weapon) = &input.weapon {
            properties["weapon"] = serde_json::json!({"type": "string", "enum": [weapon.trim()]});
        }
        if let Some(level) = input.constellation {
            properties["constellation"] =
                serde_json::json!({"type": "string", "enum": [format!("{level}凸")]});
        }
        choices.push(member);
    }
    schema["properties"]["members"]["items"] = serde_json::json!({"anyOf": choices});
    Ok(schema)
}

fn canonical_name(value: &str, mut names: impl Iterator<Item = impl AsRef<str>>) -> String {
    let normalized = normalize_asset_name(value);
    names
        .find(|name| normalize_asset_name(name.as_ref()) == normalized)
        .map(|name| name.as_ref().to_owned())
        .unwrap_or_else(|| value.trim().to_owned())
}

// Only recognized catalog names followed by a complete rank annotation are split.
// Alternatives, character readings and ambiguous names are never guessed.
fn genshin_weapon_label(value: &str, catalog: &Catalog) -> (String, Option<u8>) {
    let normalized = normalize_asset_name(value);
    for weapon in &catalog.weapons {
        let name = normalize_asset_name(&weapon.name);
        let Some(suffix) = normalized.strip_prefix(&name) else {
            continue;
        };
        if suffix.is_empty() {
            return (weapon.name.clone(), None);
        }
        let suffix = suffix
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(suffix);
        for rank in 1..=5 {
            if suffix.eq_ignore_ascii_case(&format!("R{rank}"))
                || suffix == format!("精錬{rank}")
                || suffix == format!("精錬ランク{rank}")
                || suffix == format!("{}凸", rank - 1)
                || (rank == 1 && suffix == "無凸")
            {
                return (weapon.name.clone(), Some(rank));
            }
        }
    }
    (value.trim().to_owned(), None)
}

fn merge_refinement(rank: Option<u8>, annotation: Option<u8>) -> Result<Option<u8>, String> {
    if rank
        .zip(annotation)
        .is_some_and(|(rank, annotation)| rank != annotation)
    {
        return Err("武器名の精錬表記と精錬ランクが一致しません".into());
    }
    Ok(rank.or(annotation))
}

impl ResearchIntake {
    pub(crate) fn normalize(&mut self) -> Result<(), String> {
        if self.game == GameId::Genshin {
            let catalog = crate::catalog::load_embedded_catalog().map_err(|e| e.to_string())?;
            for member in &mut self.members {
                member.name =
                    canonical_name(&member.name, catalog.characters.iter().map(|c| &c.name));
                if let Some(weapon) = &member.weapon {
                    let (name, rank) = genshin_weapon_label(weapon, &catalog);
                    member.refinement = merge_refinement(member.refinement, rank)?;
                    member.weapon = Some(name);
                }
            }
        }
        self.validate()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.members.len() > 4 {
            return Err("調査対象は4人までです".into());
        }
        let catalog = if self.game == GameId::StarRail {
            Some(crate::star_rail::load_star_rail_catalog()?)
        } else {
            None
        };
        if let Some(catalog) = &catalog {
            crate::star_rail::validate_character_combination(
                self.members.iter().map(|member| member.name.as_str()),
                catalog,
            )?;
        }
        let mut names = HashSet::new();
        for (index, member) in self.members.iter().enumerate() {
            if let Some(catalog) = &catalog {
                crate::star_rail::validate_input(member, catalog)?;
            } else if member.relics.is_some() {
                return Err("原神にはスターレイルの遺物条件を指定できません".into());
            }
            if member.slot_index != index as u8 {
                return Err("slotIndexは0からの並び順と一致させてください".into());
            }
            if member.name.trim().is_empty() {
                return Err("キャラクター名は空にできません".into());
            }
            if !names.insert(member.name.trim().to_lowercase()) {
                return Err("同じキャラクターを重複指定できません".into());
            }
            if member.constellation.is_some_and(|value| value > 6) {
                return Err("命ノ星座は0〜6で指定してください".into());
            }
            if member
                .refinement
                .is_some_and(|value| !(1..=5).contains(&value))
            {
                return Err("精錬ランクは1〜5で指定してください".into());
            }
        }
        if self.ready_to_research != (self.members.len() == 4) {
            return Err("4人揃った場合だけ調査開始可能にしてください".into());
        }
        Ok(())
    }
}

impl ResearchedTeamDraft {
    pub(crate) fn normalize_for_intake(&mut self, intake: &ResearchIntake) -> Result<(), String> {
        intake.validate()?;
        if self.game != intake.game {
            return Err("調査結果のゲームが一致しません".into());
        }
        // Array order is transport detail. slotIndex remains the requested placement.
        self.members.sort_by_key(|member| member.slot_index);
        if self.game == GameId::Genshin {
            let catalog = crate::catalog::load_embedded_catalog().map_err(|e| e.to_string())?;
            for member in &mut self.members {
                member.name =
                    canonical_name(&member.name, catalog.characters.iter().map(|c| &c.name));
                if let Some(character) = catalog.characters.iter().find(|c| c.name == member.name) {
                    member.id = character.id.clone();
                }
                let (weapon, rank) = genshin_weapon_label(&member.weapon, &catalog);
                member.refinement = merge_refinement(member.refinement, rank)?;
                member.weapon = weapon;
            }
        }
        self.validate_for_members(&intake.members)
    }

    pub fn validate_for_members(&self, requested: &[ResearchMemberInput]) -> Result<(), String> {
        self.validate()?;
        if requested.len() != self.members.len() {
            return Err("指定した人数と調査結果が一致しません".into());
        }
        for (input, member) in requested.iter().zip(&self.members) {
            if self.game == GameId::StarRail {
                crate::star_rail::validate_build(member, input, &self.sources)?;
            } else if member.star_rail.is_some() || input.relics.is_some() {
                return Err("異なるゲームの装備結果です".into());
            }
            if input.name.trim() != member.name.trim() {
                return Err(format!(
                    "指定キャラクターが変更されています（指定: {}、結果: {}）",
                    input.name, member.name
                ));
            }
            if let Some(weapon) = &input.weapon
                && weapon.trim() != member.weapon.trim()
            {
                return Err(format!(
                    "{}の指定武器が変更されています（指定: {}、結果: {}）",
                    input.name, weapon, member.weapon
                ));
            }
            if self.game == GameId::Genshin
                && let Some(rank) = input.refinement
                && member.refinement != Some(rank)
            {
                return Err(format!(
                    "{}の指定精錬ランクが変更されています（指定: R{}、結果: {:?}）",
                    input.name, rank, member.refinement
                ));
            }
            if let Some(level) = input.constellation {
                let value = member.constellation.trim();
                let matches = value == format!("{level}凸")
                    || value == level.to_string()
                    || value.eq_ignore_ascii_case(&format!("C{level}"))
                    || (level == 0 && value == "無凸")
                    || (level == 6 && value == "完凸");
                if !matches {
                    return Err(format!(
                        "{}の指定した命ノ星座が変更されています",
                        input.name
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.game == GameId::StarRail {
            crate::star_rail::validate_character_combination(
                self.members.iter().map(|member| member.name.as_str()),
                &crate::star_rail::load_star_rail_catalog()?,
            )?;
        }
        if self.game == GameId::StarRail
            && self
                .team_reasoning
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err("編成全体の採用理由・支援の分担がありません".into());
        }
        if self.title.trim().is_empty() || self.title.chars().count() > 80 {
            return Err("編成名は1〜80文字で指定してください".into());
        }
        if self.members.len() != 4 {
            return Err("調査結果には4人が必要です".into());
        }
        let mut names = HashSet::new();
        for (index, member) in self.members.iter().enumerate() {
            if member
                .refinement
                .is_some_and(|rank| !(1..=5).contains(&rank))
            {
                return Err("調査結果の精錬ランクは1〜5で指定してください".into());
            }
            if member.slot_index != index as u8 {
                return Err("調査結果のslotIndexが不正です".into());
            }
            if member.name.trim().is_empty()
                || member.weapon.trim().is_empty()
                || member.artifact.trim().is_empty()
                || member.main_stats.trim().is_empty()
                || member.sub_stats.trim().is_empty()
            {
                return Err(format!("{}のビルド情報が不足しています", member.name));
            }
            if !names.insert(member.name.trim().to_lowercase()) {
                return Err("調査結果に同じキャラクターが重複しています".into());
            }
            if !(2..=8).contains(&member.target_stats.len()) {
                return Err(format!("{}の目標ステータスは2〜8件必要です", member.name));
            }
            if member
                .target_stats
                .iter()
                .any(|stat| stat.label.trim().is_empty() || stat.value.trim().is_empty())
            {
                return Err(format!(
                    "{}の目標ステータスに名前または数値がありません",
                    member.name
                ));
            }
            if !member.target_stats.iter().any(|stat| stat.primary) {
                return Err(format!(
                    "{}に攻撃力・HP・防御力・元素熟知・基礎攻撃力などの主参照ステータスがありません",
                    member.name
                ));
            }
            for image_url in [
                member.image_url.as_deref(),
                member.weapon_image_url.as_deref(),
                member.artifact_image_url.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                validate_https_url(image_url, "画像URL")?;
            }
        }
        if self.sources.is_empty() {
            return Err("根拠ページがありません".into());
        }
        let mut source_urls = HashSet::new();
        for source in &self.sources {
            if source.title.trim().is_empty() {
                return Err("根拠ページのタイトルは必須です".into());
            }
            let normalized = crate::source_policy::normalize_source_url_for(self.game, &source.url)
                .map_err(|error| error.to_string())?;
            if !crate::source_policy::is_direct_content_url_for(self.game, &source.url)
                .map_err(|error| error.to_string())?
            {
                return Err(format!("個別本文ページではありません: {}", source.url));
            }
            if !source_urls.insert(normalized) {
                return Err(format!("根拠URLが重複しています: {}", source.url));
            }
        }
        Ok(())
    }
}

fn validate_https_url(raw: &str, label: &str) -> Result<(), String> {
    let url = Url::parse(raw).map_err(|_| format!("{label}を解析できません"))?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() {
        return Err(format!(
            "{label}はuserinfoを含まないHTTPS URLで指定してください"
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn 四人揃った受付だけ調査開始可能になる() {
        let intake = ResearchIntake {
            game: Default::default(),
            members: (0..4)
                .map(|slot_index| ResearchMemberInput {
                    relics: None,
                    slot_index,
                    name: format!("キャラ{slot_index}"),
                    weapon: None,
                    constellation: None,
                    refinement: None,
                })
                .collect(),
            missing_fields: vec!["武器".into()],
            ready_to_research: true,
        };

        assert!(intake.validate().is_ok());
    }

    fn sample_draft() -> ResearchedTeamDraft {
        let member = ResearchedTeamMember {
            refinement: Some(1),
            star_rail: None,
            slot_index: 0,
            id: "sample".into(),
            name: "サンプル".into(),
            element: "炎".into(),
            role: "アタッカー".into(),
            constellation: "無凸".into(),
            image_url: None,
            weapon: "武器".into(),
            weapon_image_url: None,
            artifact: "聖遺物".into(),
            artifact_image_url: None,
            main_stats: "攻撃力 / 炎元素ダメージ / 会心".into(),
            sub_stats: "会心率 ＞ 会心ダメージ".into(),
            target_stats: vec![
                ResearchedTargetStat {
                    label: "会心率".into(),
                    value: "70%以上".into(),
                    primary: false,
                    note: None,
                },
                ResearchedTargetStat {
                    label: "会心ダメージ".into(),
                    value: "180%以上".into(),
                    primary: false,
                    note: None,
                },
            ],
        };
        ResearchedTeamDraft {
            game: Default::default(),
            team_reasoning: None,
            title: "サンプル編成".into(),
            game_version: "6.0".into(),
            members: (0..4)
                .map(|slot_index| ResearchedTeamMember {
                    star_rail: None,
                    slot_index,
                    id: format!("sample-{slot_index}"),
                    name: format!("サンプル{slot_index}"),
                    ..member.clone()
                })
                .collect(),
            sources: vec![ResearchSource {
                title: "根拠".into(),
                url: "https://game8.jp/genshin/12345".into(),
            }],
            warnings: Vec::new(),
        }
    }

    // Synthetic advice: this tests the transport/conditions contract, not game values.
    pub(crate) fn reported_team() -> (ResearchIntake, ResearchedTeamDraft) {
        let mut draft = sample_draft();
        for (member, (name, weapon)) in draft.members.iter_mut().zip([
            ("コロンビーナ", "龍殺しの英傑"),
            ("イルーガ", "西風長槍"),
            ("リンネア", "西風猟弓"),
            ("兹白", "蝶の羽化"),
        ]) {
            member.name = name.into();
            member.weapon = weapon.into();
            member.target_stats[0].primary = true;
        }
        draft.members[3].constellation = "1凸".into();
        let intake = ResearchIntake {
            game: GameId::Genshin,
            members: draft
                .members
                .iter()
                .map(|member| ResearchMemberInput {
                    slot_index: member.slot_index,
                    name: member.name.clone(),
                    weapon: (member.slot_index == 3).then(|| member.weapon.clone()),
                    constellation: (member.slot_index == 3).then_some(1),
                    refinement: (member.slot_index == 3).then_some(1),
                    relics: None,
                })
                .collect(),
            missing_fields: Vec::new(),
            ready_to_research: true,
        };
        (intake, draft)
    }

    #[test]
    fn 報告された編成の武器表記差と配列順を正規化して条件を維持する() {
        let (intake, original) = reported_team();
        for label in [
            "蝶の羽化（R１）",
            " 蝶の羽化 (精錬1) ",
            "蝶の羽化（無凸）",
            "蝶の羽化",
        ] {
            let mut draft = original.clone();
            draft.members[3].weapon = label.into();
            draft.members.swap(0, 3);
            draft.normalize_for_intake(&intake).unwrap();
            assert_eq!(draft.members[3].id, "shihak");
            assert_eq!(draft.members[3].name, "兹白");
            assert_eq!(draft.members[3].weapon, "蝶の羽化");
            assert_eq!(draft.members[3].constellation, "1凸");
            assert_eq!(draft.members[3].refinement, Some(1));
            assert!(intake.members[..3].iter().all(|m| m.weapon.is_none()
                && m.constellation.is_none()
                && m.refinement.is_none()));
        }
    }

    #[test]
    fn 正規化は別キャラ武器精錬凸や矛盾する注釈を黙認しない() {
        let (intake, original) = reported_team();
        for (field, value) in [
            ("name", "鍾離"),
            ("name", "しはく"),
            ("name", "茲白"),
            ("weapon", "西風剣"),
            ("weapon", "蝶の羽化/西風剣"),
            ("weapon", "蝶の羽化（R5）"),
            ("constellation", "2凸"),
        ] {
            let mut value_draft = serde_json::to_value(&original).unwrap();
            value_draft["members"][3][field] = serde_json::json!(value);
            let mut draft: ResearchedTeamDraft = serde_json::from_value(value_draft).unwrap();
            assert!(
                draft.normalize_for_intake(&intake).is_err(),
                "{field}: {value}"
            );
        }
        for rank in [None, Some(2)] {
            let mut draft = original.clone();
            draft.members[3].refinement = rank;
            assert!(
                draft
                    .normalize_for_intake(&intake)
                    .unwrap_err()
                    .contains("精錬")
            );
        }
        let mut draft = original;
        draft.members[0].slot_index = 3;
        assert!(draft.normalize_for_intake(&intake).is_err());
    }

    #[test]
    fn 武器の無凸をr1へ分離しキャラの凸と未指定を維持する() {
        let (mut intake, _) = reported_team();
        intake.members[3].weapon = Some("蝶の羽化（無凸）".into());
        intake.members[3].refinement = None;
        intake.normalize().unwrap();
        assert_eq!(intake.members[3].weapon.as_deref(), Some("蝶の羽化"));
        assert_eq!(intake.members[3].refinement, Some(1));
        assert_eq!(intake.members[3].constellation, Some(1));
        assert!(
            intake.members[..3]
                .iter()
                .all(|member| member.refinement.is_none())
        );
        intake.members[3].weapon = Some("蝶の羽化（R5）".into());
        assert!(intake.normalize().is_err());
    }

    #[test]
    fn 調査schemaは今回の指定だけ固定し未指定ビルドを開放する() {
        let (intake, _) = reported_team();
        let schema = team_research_output_schema_for_intake(&intake).unwrap();
        let choices = schema["properties"]["members"]["items"]["anyOf"]
            .as_array()
            .unwrap();
        let fixed = &choices[3]["properties"];
        assert_eq!(fixed["name"]["enum"], serde_json::json!(["兹白"]));
        assert_eq!(fixed["id"]["enum"], serde_json::json!(["shihak"]));
        assert_eq!(fixed["weapon"]["enum"], serde_json::json!(["蝶の羽化"]));
        assert_eq!(fixed["refinement"]["enum"], serde_json::json!([1]));
        assert_eq!(fixed["constellation"]["enum"], serde_json::json!(["1凸"]));
        for key in [
            "weapon",
            "refinement",
            "constellation",
            "artifact",
            "targetStats",
        ] {
            assert!(choices[0]["properties"][key].get("enum").is_none(), "{key}");
        }
        let (other, _) = crate::star_rail::tests::sample();
        let schema = team_research_output_schema_for_intake(&other).unwrap();
        let fixed = &schema["properties"]["members"]["items"]["anyOf"][0]["properties"];
        assert_eq!(
            fixed["starRail"]["properties"]["superimposition"]["enum"],
            serde_json::json!([1])
        );
    }

    #[test]
    fn 旧保存結果の精錬欠落は読み込み可能で新規の固定精錬には使わない() {
        let (intake, draft) = reported_team();
        let mut value = serde_json::to_value(&draft).unwrap();
        for member in value["members"].as_array_mut().unwrap() {
            member.as_object_mut().unwrap().remove("refinement");
        }
        let restored: ResearchedTeamDraft = serde_json::from_value(value).unwrap();
        restored.validate().unwrap();
        assert!(restored.validate_for_members(&intake.members).is_err());
    }

    #[test]
    fn 主参照ステータスがない調査結果を拒否する() {
        let draft = sample_draft();
        assert!(draft.validate().unwrap_err().contains("主参照ステータス"));
    }

    #[test]
    fn 空欄で件数を埋めた目標ステータスを拒否する() {
        let mut draft = sample_draft();
        for member in &mut draft.members {
            member.target_stats[0].label = "攻撃力".into();
            member.target_stats[0].value = "2,000".into();
            member.target_stats[0].primary = true;
        }
        assert!(draft.validate().is_ok());
        for (label, value) in [("", "100%"), ("unused", ""), ("元素チャージ効率", " \t")] {
            draft.members[3].target_stats[1].label = label.into();
            draft.members[3].target_stats[1].value = value.into();
            assert!(draft.validate().unwrap_err().contains("名前または数値"));
        }
    }

    #[test]
    fn 指定条件を資料の内容で上書きした調査結果を拒否する() {
        let mut draft = sample_draft();
        for member in &mut draft.members {
            member.target_stats[0].primary = true;
        }
        let requested = draft
            .members
            .iter()
            .map(|member| ResearchMemberInput {
                relics: None,
                slot_index: member.slot_index,
                name: member.name.clone(),
                weapon: Some(member.weapon.clone()),
                constellation: Some(0),
                refinement: Some(1),
            })
            .collect::<Vec<_>>();
        for value in ["無凸", "0凸", "C0", "0"] {
            draft.members[0].constellation = value.into();
            assert!(draft.validate_for_members(&requested).is_ok());
        }
        draft.members[0].constellation = "1凸".into();
        assert!(
            draft
                .validate_for_members(&requested)
                .unwrap_err()
                .contains("命ノ星座")
        );
        draft.members[0].constellation = "無凸".into();
        draft.members[0].weapon = "別の武器".into();
        assert!(
            draft
                .validate_for_members(&requested)
                .unwrap_err()
                .contains("武器")
        );
        draft.members[0].weapon = requested[0].weapon.clone().unwrap();
        draft.members[0].name = "別のキャラクター".into();
        assert!(
            draft
                .validate_for_members(&requested)
                .unwrap_err()
                .contains("キャラクター")
        );
    }
}
