use crate::domain::make_strict_output_schema;
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
    #[schemars(length(max = 4))]
    pub members: Vec<ResearchMemberInput>,
    #[schemars(length(max = 16))]
    pub missing_fields: Vec<String>,
    pub ready_to_research: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchConversation {
    pub session_id: String,
    pub status: ResearchConversationStatus,
    pub messages: Vec<ResearchMessage>,
    pub members: Vec<ResearchMemberInput>,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchedTeamDraft {
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
    pub team_id: String,
    pub title: String,
    pub member_names: Vec<String>,
    pub member_image_urls: Vec<Option<String>>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OnDemandResearchProgress {
    pub session_id: String,
    pub stage: String,
    pub detail: String,
    pub member_name: Option<String>,
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

impl ResearchIntake {
    pub fn validate(&self) -> Result<(), String> {
        if self.members.len() > 4 {
            return Err("調査対象は4人までです".into());
        }
        let mut names = HashSet::new();
        for (index, member) in self.members.iter().enumerate() {
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
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() || self.title.chars().count() > 80 {
            return Err("編成名は1〜80文字で指定してください".into());
        }
        if self.members.len() != 4 {
            return Err("調査結果には4人が必要です".into());
        }
        let mut names = HashSet::new();
        for (index, member) in self.members.iter().enumerate() {
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
            let normalized = crate::source_policy::normalize_source_url(&source.url)
                .map_err(|error| error.to_string())?;
            if !crate::source_policy::is_direct_content_url(&source.url)
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
mod tests {
    use super::*;

    #[test]
    fn 四人揃った受付だけ調査開始可能になる() {
        let intake = ResearchIntake {
            members: (0..4)
                .map(|slot_index| ResearchMemberInput {
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

    #[test]
    fn 主参照ステータスがない調査結果を拒否する() {
        let member = ResearchedTeamMember {
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
        let draft = ResearchedTeamDraft {
            title: "サンプル編成".into(),
            game_version: "6.0".into(),
            members: (0..4)
                .map(|slot_index| ResearchedTeamMember {
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
        };

        assert!(draft.validate().unwrap_err().contains("主参照ステータス"));
    }
}
