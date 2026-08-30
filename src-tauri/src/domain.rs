use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

use crate::source_policy::normalize_source_url;

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }
    };
}

string_enum!(BuildIntent {
    Auto,
    OnFieldDamage,
    OffFieldDamage,
    ReactionTrigger,
    Support,
    Sustain,
});
string_enum!(ReactionOwnership {
    Main,
    Partial,
    None,
    Unknown,
});
string_enum!(EnergyPriority {
    DamageFirst,
    Balanced,
    BurstStability,
});
string_enum!(SurvivabilityPriority { Normal, High });
string_enum!(TargetScope {
    CharacterSheetUnbuffed,
    CharacterSheetWithStaticTeamEffects,
    InCombatConditional,
});
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
pub enum EvidenceGrade {
    A,
    B,
    C,
}
string_enum!(EvidenceVerification {
    HostExactMatch,
    HostFuzzyMatch,
    UrlEventOnly,
    Unverified,
});
string_enum!(ResolutionStatus {
    Resolved,
    NeedsUserChoice,
    Unresolved,
});
string_enum!(AnalysisStatus {
    Queued,
    StartingCodex,
    Researching,
    VerifyingSources,
    Reconciling,
    Solving,
    Persisting,
    Succeeded,
    Failed,
    Cancelled,
    Superseded,
    Abandoned,
});
string_enum!(ResultValidity {
    Current,
    SoftStale,
    HardStale,
    Invalid,
});
string_enum!(ConditionOperator {
    Equals,
    NotEquals,
    Includes,
    Gte,
    Lte,
});
string_enum!(StatUnit { Flat, Percent });
string_enum!(EvidenceClaimType {
    ArtifactPlan,
    MainStatPackage,
    SubstatPriority,
    TargetStat,
    Role,
    TeamInteraction,
});

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum ResearchSchemaVersion {
    #[serde(rename = "character-research-v1")]
    V1,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterBuildIntent {
    pub role: BuildIntent,
    pub reaction_ownership: ReactionOwnership,
    pub energy_priority: EnergyPriority,
    pub survivability_priority: SurvivabilityPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartyMemberInput {
    #[schemars(range(min = 0, max = 3))]
    pub slot_index: u8,
    pub character_id: String,
    pub weapon_id: String,
    #[schemars(range(min = 1, max = 5))]
    pub refinement: u8,
    #[schemars(range(min = 0, max = 6))]
    pub constellation: u8,
    pub intent: CharacterBuildIntent,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisVersions {
    pub catalog_version: String,
    pub source_policy_version: String,
    pub prompt_version: String,
    pub schema_version: String,
    pub reconciler_version: String,
    pub solver_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedAssumptions {
    pub character_level: u8,
    pub weapon_level: u8,
    pub artifact_level: u8,
    pub artifact_rarity: u8,
    pub sheet_timing: String,
    pub final_ascension: bool,
    pub all_talents_available: bool,
    pub witch_teaching_when_applicable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisInput {
    pub party_id: String,
    pub party_name: String,
    pub game_version: String,
    pub members: [PartyMemberInput; 4],
    pub assumptions: FixedAssumptions,
    pub versions: AnalysisVersions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ArtifactHalf {
    ExactSet { set_id: String },
    EffectGroup { effect_group_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ArtifactPlan {
    FourPiece {
        set_id: String,
    },
    TwoPlusTwo {
        first: ArtifactHalf,
        second: ArtifactHalf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum ConditionValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildCondition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: ConditionValue,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatPriority {
    pub stat: String,
    #[schemars(range(min = 1))]
    pub rank: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetStatRange {
    pub stat: String,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub unit: StatUnit,
    pub scope: TargetScope,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MainStatPackage {
    pub id: String,
    pub sands: String,
    pub goblet: String,
    pub circlet: String,
    #[schemars(length(max = 16))]
    pub conditions: Vec<BuildCondition>,
    #[schemars(length(min = 1, max = 32))]
    pub substat_priority: Vec<StatPriority>,
    #[schemars(length(max = 16))]
    pub target_stats: Vec<TargetStatRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceLocator {
    pub heading: Option<String>,
    pub section: Option<String>,
    pub text_fragment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceEvidence {
    pub source_page_id: String,
    pub evidence_excerpt: Option<String>,
    pub evidence_summary: String,
    pub locator: Option<EvidenceLocator>,
    pub verification: EvidenceVerification,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NormalizedClaimValue {
    ArtifactPlan { value: ArtifactPlan },
    MainStatPackage { value: MainStatPackage },
    SubstatPriority { value: Vec<StatPriority> },
    TargetStat { value: TargetStatRange },
    Role { value: CharacterBuildIntent },
    TeamInteraction { value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceClaim {
    pub claim_type: EvidenceClaimType,
    pub normalized_value: NormalizedClaimValue,
    pub conditions: Vec<BuildCondition>,
    pub evidence: SourceEvidence,
    pub evidence_grade: EvidenceGrade,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildVariant {
    pub id: String,
    pub character_id: String,
    pub artifact_plan: ArtifactPlan,
    pub main_stat_package: MainStatPackage,
    pub conditions: Vec<BuildCondition>,
    pub team_buff_keys: Vec<String>,
    pub evidence_claims: Vec<EvidenceClaim>,
    pub source_family_count: usize,
    pub conflict_penalty: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterBuildResolution {
    pub character_id: String,
    pub selected_variant_id: Option<String>,
    pub alternatives: Vec<BuildVariant>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamBuildResolution {
    pub status: ResolutionStatus,
    pub members: Vec<CharacterBuildResolution>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostGeneratedIdentity {
    pub analysis_run_id: String,
    pub party_composition_hash: String,
    pub analysis_input_hash: String,
    pub evidence_snapshot_hash: String,
    pub result_hash: String,
    pub checked_at: String,
    pub created_at: String,
}

/// Codexから受け取る未信頼な参照ページ。ID・検証結果・hashはRust側で付与する。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchSourcePage {
    #[schemars(length(min = 1, max = 2048))]
    pub source_url: String,
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 100))]
    pub publisher: String,
    #[schemars(length(min = 1, max = 32))]
    pub game_version: String,
    pub updated_at: Option<String>,
}

/// Codexから受け取る根拠。host由来のsourcePageId等を意図的に含めない。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchEvidence {
    #[schemars(length(min = 1, max = 2048))]
    pub source_url: String,
    pub evidence_excerpt: Option<String>,
    #[schemars(length(min = 1, max = 1000))]
    pub evidence_summary: String,
    pub locator: Option<ResearchLocator>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchLocator {
    pub heading: Option<String>,
    pub section: Option<String>,
    pub text_fragment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchClaim {
    pub claim_type: EvidenceClaimType,
    pub normalized_value: NormalizedClaimValue,
    #[schemars(length(max = 16))]
    pub conditions: Vec<BuildCondition>,
    pub evidence: ResearchEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchBuildVariant {
    #[schemars(length(min = 1, max = 100))]
    pub id: String,
    pub artifact_plan: ArtifactPlan,
    pub main_stat_package: MainStatPackage,
    #[schemars(length(max = 16))]
    pub conditions: Vec<BuildCondition>,
    #[schemars(length(max = 16))]
    pub team_buff_keys: Vec<String>,
    #[schemars(length(min = 3, max = 48))]
    pub claims: Vec<ResearchClaim>,
}

/// 1キャラクター分のCodex構造化出力契約。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterResearchOutput {
    pub schema_version: ResearchSchemaVersion,
    #[schemars(length(min = 1, max = 100))]
    pub character_id: String,
    #[schemars(length(min = 1, max = 12))]
    pub sources: Vec<ResearchSourcePage>,
    #[schemars(length(min = 1, max = 3))]
    pub variants: Vec<ResearchBuildVariant>,
    #[schemars(length(max = 16))]
    pub warnings: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainValidationError {
    #[error("分析契約の検証に失敗しました: {0}")]
    Invalid(String),
}

pub fn validate_analysis_input(input: &AnalysisInput) -> Result<(), DomainValidationError> {
    if input.party_id.trim().is_empty()
        || input.party_name.trim().is_empty()
        || input.game_version.trim().is_empty()
    {
        return Err(invalid("編成ID・編成名・ゲーム版は必須です"));
    }
    if input.assumptions.character_level != 90
        || input.assumptions.weapon_level != 90
        || input.assumptions.artifact_level != 20
        || input.assumptions.artifact_rarity != 5
        || input.assumptions.sheet_timing != "pre_combat"
        || !input.assumptions.final_ascension
        || !input.assumptions.all_talents_available
        || !input.assumptions.witch_teaching_when_applicable
    {
        return Err(invalid("MVPの固定前提と一致しません"));
    }

    let mut characters = HashSet::new();
    let mut traveler_count = 0;
    for (index, member) in input.members.iter().enumerate() {
        if usize::from(member.slot_index) != index {
            return Err(invalid("slotIndexは0から3の並び順と一致する必要があります"));
        }
        if member.character_id.trim().is_empty() || member.weapon_id.trim().is_empty() {
            return Err(invalid("分析開始時はキャラクターと武器が必須です"));
        }
        if !(1..=5).contains(&member.refinement) || member.constellation > 6 {
            return Err(invalid("精錬ランクまたは命ノ星座の範囲が不正です"));
        }
        if !characters.insert(member.character_id.as_str()) {
            return Err(invalid("同一キャラクターを重複編成できません"));
        }
        if member.character_id.starts_with("traveler-") {
            traveler_count += 1;
        }
    }
    if traveler_count > 1 {
        return Err(invalid("旅人の異なる元素バリアントを同時編成できません"));
    }
    Ok(())
}

pub fn validate_character_research_output(
    output: &CharacterResearchOutput,
    expected_character_id: &str,
    expected_game_version: &str,
) -> Result<(), DomainValidationError> {
    if output.character_id != expected_character_id {
        return Err(invalid("調査対象とcharacterIdが一致しません"));
    }
    if !(1..=12).contains(&output.sources.len()) {
        return Err(invalid("sourcesは1件以上12件以下である必要があります"));
    }
    if !(1..=3).contains(&output.variants.len()) {
        return Err(invalid("variantsは1件以上3件以下である必要があります"));
    }
    if output.warnings.len() > 16 {
        return Err(invalid("warningsは16件以下である必要があります"));
    }

    let mut source_urls = HashSet::new();
    for source in &output.sources {
        if source.source_url.trim().is_empty()
            || source.title.trim().is_empty()
            || source.publisher.trim().is_empty()
            || source.game_version.trim().is_empty()
        {
            return Err(invalid("参照ページの必須項目が空です"));
        }
        if source.game_version != expected_game_version {
            return Err(invalid("根拠ページのゲーム版が分析対象と一致しません"));
        }
        let normalized_url = normalize_source_url(&source.source_url)
            .map_err(|error| invalid(format!("根拠URLが不正です: {error}")))?;
        if !source_urls.insert(normalized_url) {
            return Err(invalid("同じ参照URLを重複登録できません"));
        }
    }

    let mut variant_ids = HashSet::new();
    for variant in &output.variants {
        if variant.id.trim().is_empty() || !variant_ids.insert(variant.id.as_str()) {
            return Err(invalid("BuildVariantのIDは空でない一意値が必要です"));
        }
        validate_artifact_plan(&variant.artifact_plan)?;
        validate_main_stat_package(&variant.main_stat_package)?;
        if !(3..=48).contains(&variant.claims.len()) {
            return Err(invalid(
                "各候補のclaimsは3件以上48件以下である必要があります",
            ));
        }

        let mut required_claims = HashSet::new();
        let mut unique_claims = HashSet::new();
        for claim in &variant.claims {
            if claim.claim_type != claim.normalized_value.claim_type() {
                return Err(invalid("claimTypeとnormalizedValue.kindが一致しません"));
            }
            let evidence_url = normalize_source_url(&claim.evidence.source_url)
                .map_err(|error| invalid(format!("根拠URLが不正です: {error}")))?;
            if !source_urls.contains(&evidence_url) {
                return Err(invalid("根拠URLがsourcesに含まれていません"));
            }
            let claim_key = serde_json::to_string(&(
                claim.claim_type,
                &claim.normalized_value,
                &claim.conditions,
                evidence_url,
            ))
            .map_err(|error| invalid(format!("claimを正規化できません: {error}")))?;
            if !unique_claims.insert(claim_key) {
                return Err(invalid("同一の根拠claimを重複登録できません"));
            }
            required_claims.insert(claim.claim_type);
        }
        for required in [
            EvidenceClaimType::ArtifactPlan,
            EvidenceClaimType::MainStatPackage,
            EvidenceClaimType::SubstatPriority,
        ] {
            if !required_claims.contains(&required) {
                return Err(invalid(
                    "候補に聖遺物・メイン・サブステの必須根拠がありません",
                ));
            }
        }
    }
    Ok(())
}

impl NormalizedClaimValue {
    fn claim_type(&self) -> EvidenceClaimType {
        match self {
            Self::ArtifactPlan { .. } => EvidenceClaimType::ArtifactPlan,
            Self::MainStatPackage { .. } => EvidenceClaimType::MainStatPackage,
            Self::SubstatPriority { .. } => EvidenceClaimType::SubstatPriority,
            Self::TargetStat { .. } => EvidenceClaimType::TargetStat,
            Self::Role { .. } => EvidenceClaimType::Role,
            Self::TeamInteraction { .. } => EvidenceClaimType::TeamInteraction,
        }
    }
}

fn validate_artifact_plan(plan: &ArtifactPlan) -> Result<(), DomainValidationError> {
    match plan {
        ArtifactPlan::FourPiece { set_id } if set_id.trim().is_empty() => {
            Err(invalid("4セット構成のsetIdが空です"))
        }
        ArtifactPlan::TwoPlusTwo {
            first: ArtifactHalf::ExactSet { set_id: first },
            second: ArtifactHalf::ExactSet { set_id: second },
        } if first == second => Err(invalid("2+2構成で同一セットを重複選択できません")),
        _ => Ok(()),
    }
}

fn validate_main_stat_package(package: &MainStatPackage) -> Result<(), DomainValidationError> {
    if [
        package.id.as_str(),
        package.sands.as_str(),
        package.goblet.as_str(),
        package.circlet.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
        || package.substat_priority.is_empty()
    {
        return Err(invalid("メインステータスpackageの必須項目が空です"));
    }

    let mut ranks = HashSet::new();
    for priority in &package.substat_priority {
        if priority.stat.trim().is_empty() || priority.rank == 0 || !ranks.insert(priority.rank) {
            return Err(invalid("サブステ優先順位は空でない一意の正整数が必要です"));
        }
    }
    for target in &package.target_stats {
        if target.stat.trim().is_empty()
            || matches!((target.minimum, target.maximum), (Some(min), Some(max)) if min > max)
        {
            return Err(invalid("目標ステータスの範囲が不正です"));
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> DomainValidationError {
    DomainValidationError::Invalid(message.into())
}

pub fn character_research_output_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schema_for!(CharacterResearchOutput))
        .expect("CharacterResearchOutputのSchemaをJSON化できませんでした");
    make_strict_output_schema(&mut schema);
    schema
}

fn make_strict_output_schema(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                make_strict_output_schema(item);
            }
        }
        serde_json::Value::Object(object) => {
            object.remove("format");
            for child in object.values_mut() {
                make_strict_output_schema(child);
            }

            if let Some(variants) = object.remove("oneOf") {
                object.insert("anyOf".into(), variants);
            }
            if let Some(constant) = object.remove("const") {
                object.insert("enum".into(), serde_json::Value::Array(vec![constant]));
            }

            let property_names = object
                .get("properties")
                .and_then(serde_json::Value::as_object)
                .map(|properties| properties.keys().cloned().collect::<Vec<_>>());
            if let Some(property_names) = property_names {
                object.insert(
                    "required".into(),
                    serde_json::Value::Array(
                        property_names
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
                object.insert(
                    "additionalProperties".into(),
                    serde_json::Value::Bool(false),
                );
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn 列挙値をcamel_caseのフィールドとsnake_caseの値で往復できる() {
        let intent = CharacterBuildIntent {
            role: BuildIntent::OffFieldDamage,
            reaction_ownership: ReactionOwnership::Partial,
            energy_priority: EnergyPriority::BurstStability,
            survivability_priority: SurvivabilityPriority::High,
        };
        let value = serde_json::to_value(&intent).expect("serialize");
        assert_eq!(value["role"], "off_field_damage");
        assert_eq!(value["reactionOwnership"], "partial");
        assert_eq!(
            serde_json::from_value::<CharacterBuildIntent>(value).unwrap(),
            intent
        );
    }

    #[test]
    fn 構造化出力schemaは候補数と追加フィールドを制限する() {
        let schema = character_research_output_schema();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["variants"]["minItems"], 1);
        assert_eq!(schema["properties"]["variants"]["maxItems"], 3);
        assert!(schema["$defs"]["ResearchBuildVariant"]["additionalProperties"] == json!(false));
        assert!(
            schema["$defs"]["ResearchSourcePage"]["properties"]["updatedAt"]["type"]
                .as_array()
                .is_some_and(|variants| variants.iter().any(|value| value == "null"))
        );
        assert_eq!(
            schema["$defs"]["ResearchSourcePage"]["required"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        let serialized = serde_json::to_string(&schema).unwrap();
        assert!(!serialized.contains("\"oneOf\""));
        assert!(!serialized.contains("\"const\""));
    }

    #[test]
    fn codex出力にホスト生成値を要求しない() {
        let schema = character_research_output_schema();
        for forbidden in [
            "analysisRunId",
            "jobId",
            "sourcePageId",
            "contentHash",
            "resultHash",
            "checkedAt",
            "createdAt",
        ] {
            assert!(
                !schema_has_property(&schema, forbidden),
                "{forbidden}を含んでいます"
            );
        }
    }

    fn schema_has_property(value: &serde_json::Value, property_name: &str) -> bool {
        match value {
            serde_json::Value::Array(items) => items
                .iter()
                .any(|item| schema_has_property(item, property_name)),
            serde_json::Value::Object(object) => {
                object
                    .get("properties")
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|properties| properties.contains_key(property_name))
                    || object
                        .values()
                        .any(|item| schema_has_property(item, property_name))
            }
            _ => false,
        }
    }

    #[test]
    fn 分析入力の固定前提と編成重複を検証する() {
        let member = |slot_index: u8, character_id: &str| {
            json!({
                "slotIndex": slot_index,
                "characterId": character_id,
                "weaponId": format!("weapon-{slot_index}"),
                "refinement": 1,
                "constellation": 0,
                "intent": {
                    "role": "auto",
                    "reactionOwnership": "unknown",
                    "energyPriority": "balanced",
                    "survivabilityPriority": "normal"
                }
            })
        };
        let mut value = json!({
            "partyId": "party-1",
            "partyName": "検証編成",
            "gameVersion": "7.0",
            "members": [
                member(0, "char-a"),
                member(1, "char-b"),
                member(2, "char-c"),
                member(3, "char-d")
            ],
            "assumptions": {
                "characterLevel": 90,
                "weaponLevel": 90,
                "artifactLevel": 20,
                "artifactRarity": 5,
                "sheetTiming": "pre_combat",
                "finalAscension": true,
                "allTalentsAvailable": true,
                "witchTeachingWhenApplicable": true
            },
            "versions": {
                "catalogVersion": "catalog-v1",
                "sourcePolicyVersion": "source-v1",
                "promptVersion": "prompt-v1",
                "schemaVersion": "schema-v1",
                "reconcilerVersion": "reconciler-v1",
                "solverVersion": "solver-v1"
            }
        });
        let input: AnalysisInput = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(validate_analysis_input(&input), Ok(()));

        value["members"][1]["characterId"] = json!("char-a");
        let duplicate: AnalysisInput = serde_json::from_value(value).unwrap();
        assert!(validate_analysis_input(&duplicate).is_err());

        let mut invalid_range = duplicate;
        invalid_range.members[0].character_id = "char-z".into();
        invalid_range.members[0].refinement = 0;
        assert!(validate_analysis_input(&invalid_range).is_err());
    }

    #[test]
    fn codex出力の必須根拠と参照urlを検証する() {
        let package = json!({
            "id": "main-1",
            "sands": "攻撃力%",
            "goblet": "元素ダメージ",
            "circlet": "会心率",
            "conditions": [],
            "substatPriority": [{ "stat": "会心率", "rank": 1 }],
            "targetStats": []
        });
        let evidence = json!({
            "sourceUrl": "https://wikiwiki.jp/genshinwiki/example",
            "evidenceExcerpt": null,
            "evidenceSummary": "検証用の要約",
            "locator": null
        });
        let output: CharacterResearchOutput = serde_json::from_value(json!({
            "schemaVersion": "character-research-v1",
            "characterId": "char-a",
            "sources": [{
                "sourceUrl": "https://wikiwiki.jp/genshinwiki/example",
                "title": "検証ページ",
                "publisher": "原神 Wiki",
                "gameVersion": "7.0",
                "updatedAt": null
            }],
            "variants": [{
                "id": "variant-a",
                "artifactPlan": { "type": "four_piece", "setId": "set-a" },
                "mainStatPackage": package.clone(),
                "conditions": [],
                "teamBuffKeys": [],
                "claims": [
                    {
                        "claimType": "artifact_plan",
                        "normalizedValue": {
                            "kind": "artifact_plan",
                            "value": { "type": "four_piece", "setId": "set-a" }
                        },
                        "conditions": [],
                        "evidence": evidence.clone()
                    },
                    {
                        "claimType": "main_stat_package",
                        "normalizedValue": { "kind": "main_stat_package", "value": package.clone() },
                        "conditions": [],
                        "evidence": evidence.clone()
                    },
                    {
                        "claimType": "substat_priority",
                        "normalizedValue": {
                            "kind": "substat_priority",
                            "value": [{ "stat": "会心率", "rank": 1 }]
                        },
                        "conditions": [],
                        "evidence": evidence
                    }
                ]
            }],
            "warnings": []
        }))
        .unwrap();

        assert_eq!(
            validate_character_research_output(&output, "char-a", "7.0"),
            Ok(())
        );
        assert!(validate_character_research_output(&output, "char-b", "7.0").is_err());

        let mut version_mismatch = output.clone();
        version_mismatch.sources[0].game_version = "6.0".into();
        assert!(matches!(
            validate_character_research_output(&version_mismatch, "char-a", "7.0"),
            Err(DomainValidationError::Invalid(message)) if message.contains("ゲーム版")
        ));

        let mut duplicated = output;
        let duplicate_claim = duplicated.variants[0].claims[0].clone();
        duplicated.variants[0].claims.push(duplicate_claim);
        assert!(matches!(
            validate_character_research_output(&duplicated, "char-a", "7.0"),
            Err(DomainValidationError::Invalid(message)) if message.contains("重複登録")
        ));
    }

    #[test]
    fn codex出力の許可外urlを拒否する() {
        let output: CharacterResearchOutput = serde_json::from_value(json!({
            "schemaVersion": "character-research-v1",
            "characterId": "char-a",
            "sources": [{
                "sourceUrl": "https://evil.example/genshinwiki/a",
                "title": "不正ページ",
                "publisher": "不正",
                "gameVersion": "7.0",
                "updatedAt": null
            }],
            "variants": [{
                "id": "dummy",
                "artifactPlan": { "type": "four_piece", "setId": "set-a" },
                "mainStatPackage": {
                    "id": "main",
                    "sands": "攻撃力%",
                    "goblet": "元素ダメージ",
                    "circlet": "会心率",
                    "conditions": [],
                    "substatPriority": [{ "stat": "会心率", "rank": 1 }],
                    "targetStats": []
                },
                "conditions": [],
                "teamBuffKeys": [],
                "claims": []
            }],
            "warnings": []
        }))
        .unwrap();
        assert!(matches!(
            validate_character_research_output(&output, "char-a", "7.0"),
            Err(DomainValidationError::Invalid(message)) if message.contains("根拠URL")
        ));
    }
}
