use crate::domain;
use crate::source_policy::{SourcePolicyError, normalize_source_url};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// ホスト側で検証済みの参照ページ。
#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema, PartialEq, Eq,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifiedSourcePage {
    pub source_url: String,
    pub source_page_id: String,
    pub content_hash: Option<String>,
    pub verification: domain::EvidenceVerification,
    pub source_family: String,
    pub is_direct_content_page: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReconcilerError {
    #[error(transparent)]
    Domain(#[from] domain::DomainValidationError),
    #[error("根拠URL「{0}」に対応する検証済みページがありません")]
    UnverifiedUrl(String),
    #[error(transparent)]
    SourcePolicy(#[from] SourcePolicyError),
    #[error("正規化後の検証済みURLが重複しています: {0}")]
    DuplicateVerifiedUrl(String),
    #[error("検証済みページの本文hashがありません: {0}")]
    MissingContentHash(String),
    #[error("検索結果・一覧ページを直接根拠にはできません: {0}")]
    DirectContentRequired(String),
    #[error("候補「{0}」の本体・根拠・成立条件が矛盾しています")]
    ContradictoryVariant(String),
    #[error("候補「{0}」に本文確認済みの必須根拠が揃っていません")]
    InsufficientVerifiedEvidence(String),
    #[error("候補条件を評価できません: {0}")]
    UnsupportedCondition(String),
}

/// 旧称を利用する呼び出し側にも同じエラー型を公開する。
pub type ReconcileError = ReconcilerError;

pub fn reconcile_character_research(
    output: &domain::CharacterResearchOutput,
    verified_pages: &[VerifiedSourcePage],
    analysis_input: &domain::AnalysisInput,
) -> Result<Vec<domain::BuildVariant>, ReconcilerError> {
    domain::validate_analysis_input(analysis_input)?;
    domain::validate_character_research_output(
        output,
        &output.character_id,
        &analysis_input.game_version,
    )?;
    let member = analysis_input
        .members
        .iter()
        .find(|member| member.character_id == output.character_id)
        .ok_or_else(|| ReconcilerError::ContradictoryVariant(output.character_id.clone()))?;

    let mut verified_by_url = HashMap::new();
    for page in verified_pages {
        let normalized_url = normalize_source_url(&page.source_url)?;
        if matches!(
            page.verification,
            domain::EvidenceVerification::HostExactMatch
                | domain::EvidenceVerification::HostFuzzyMatch
        ) && page
            .content_hash
            .as_deref()
            .is_none_or(|hash| hash.trim().is_empty())
        {
            return Err(ReconcilerError::MissingContentHash(normalized_url));
        }
        if matches!(
            page.verification,
            domain::EvidenceVerification::HostExactMatch
                | domain::EvidenceVerification::HostFuzzyMatch
        ) && !page.is_direct_content_page
        {
            return Err(ReconcilerError::DirectContentRequired(normalized_url));
        }
        if page.source_family.trim().is_empty() {
            return Err(ReconcilerError::ContradictoryVariant(
                "sourceFamilyが空です".into(),
            ));
        }
        if verified_by_url
            .insert(normalized_url.clone(), page)
            .is_some()
        {
            return Err(ReconcilerError::DuplicateVerifiedUrl(normalized_url));
        }
    }

    output
        .variants
        .iter()
        .map(|variant| {
            for condition in variant
                .conditions
                .iter()
                .chain(variant.main_stat_package.conditions.iter())
            {
                if !condition_matches(condition, analysis_input, member)? {
                    return Err(ReconcilerError::ContradictoryVariant(variant.id.clone()));
                }
            }
            let mut evidence_claims = Vec::with_capacity(variant.claims.len());
            let mut source_families = HashSet::new();
            let mut verified_required_claims = HashSet::new();

            for claim in &variant.claims {
                let normalized_url = normalize_source_url(&claim.evidence.source_url)?;
                let page = verified_by_url.get(&normalized_url).ok_or_else(|| {
                    ReconcilerError::UnverifiedUrl(claim.evidence.source_url.clone())
                })?;
                let grade = evidence_grade(page.verification);
                let is_required = matches!(
                    claim.claim_type,
                    domain::EvidenceClaimType::ArtifactPlan
                        | domain::EvidenceClaimType::MainStatPackage
                        | domain::EvidenceClaimType::SubstatPriority
                );
                let mut conditions_hold = true;
                for condition in &claim.conditions {
                    conditions_hold &= condition_matches(condition, analysis_input, member)?;
                }
                if claim_conflicts_with_variant(claim, variant) || !conditions_hold {
                    return Err(ReconcilerError::ContradictoryVariant(variant.id.clone()));
                }
                if grade != domain::EvidenceGrade::C {
                    source_families.insert(page.source_family.as_str());
                    if is_required {
                        verified_required_claims.insert(claim.claim_type);
                    }
                }

                evidence_claims.push(domain::EvidenceClaim {
                    claim_type: claim.claim_type,
                    normalized_value: claim.normalized_value.clone(),
                    conditions: claim.conditions.clone(),
                    evidence: domain::SourceEvidence {
                        source_page_id: page.source_page_id.clone(),
                        evidence_excerpt: claim.evidence.evidence_excerpt.clone(),
                        evidence_summary: claim.evidence.evidence_summary.clone(),
                        locator: claim.evidence.locator.as_ref().map(|locator| {
                            domain::EvidenceLocator {
                                heading: locator.heading.clone(),
                                section: locator.section.clone(),
                                text_fragment: locator.text_fragment.clone(),
                            }
                        }),
                        verification: page.verification,
                        content_hash: page.content_hash.clone(),
                    },
                    evidence_grade: grade,
                });
            }

            if [
                domain::EvidenceClaimType::ArtifactPlan,
                domain::EvidenceClaimType::MainStatPackage,
                domain::EvidenceClaimType::SubstatPriority,
            ]
            .iter()
            .any(|claim_type| !verified_required_claims.contains(claim_type))
            {
                return Err(ReconcilerError::InsufficientVerifiedEvidence(
                    variant.id.clone(),
                ));
            }

            Ok(domain::BuildVariant {
                id: variant.id.clone(),
                character_id: output.character_id.clone(),
                artifact_plan: variant.artifact_plan.clone(),
                main_stat_package: variant.main_stat_package.clone(),
                conditions: variant.conditions.clone(),
                team_buff_keys: variant.team_buff_keys.clone(),
                evidence_claims,
                source_family_count: source_families.len(),
                conflict_penalty: 0,
            })
        })
        .collect()
}

fn evidence_grade(verification: domain::EvidenceVerification) -> domain::EvidenceGrade {
    match verification {
        domain::EvidenceVerification::HostExactMatch => domain::EvidenceGrade::A,
        domain::EvidenceVerification::HostFuzzyMatch => domain::EvidenceGrade::B,
        domain::EvidenceVerification::UrlEventOnly | domain::EvidenceVerification::Unverified => {
            domain::EvidenceGrade::C
        }
    }
}

fn condition_matches(
    condition: &domain::BuildCondition,
    input: &domain::AnalysisInput,
    member: &domain::PartyMemberInput,
) -> Result<bool, ReconcilerError> {
    let actual = match condition.field.as_str() {
        "constellation" => domain::ConditionValue::Number(f64::from(member.constellation)),
        "refinement" => domain::ConditionValue::Number(f64::from(member.refinement)),
        "role" => enum_condition_value(member.intent.role),
        "reactionOwnership" => enum_condition_value(member.intent.reaction_ownership),
        "energyPriority" => enum_condition_value(member.intent.energy_priority),
        "survivabilityPriority" => enum_condition_value(member.intent.survivability_priority),
        "characterLevel" => {
            domain::ConditionValue::Number(f64::from(input.assumptions.character_level))
        }
        "weaponLevel" => domain::ConditionValue::Number(f64::from(input.assumptions.weapon_level)),
        "artifactLevel" => {
            domain::ConditionValue::Number(f64::from(input.assumptions.artifact_level))
        }
        "artifactRarity" => {
            domain::ConditionValue::Number(f64::from(input.assumptions.artifact_rarity))
        }
        "gameVersion" => domain::ConditionValue::String(input.game_version.clone()),
        "finalAscension" => domain::ConditionValue::Boolean(input.assumptions.final_ascension),
        "allTalentsAvailable" => {
            domain::ConditionValue::Boolean(input.assumptions.all_talents_available)
        }
        "witchTeachingWhenApplicable" => {
            domain::ConditionValue::Boolean(input.assumptions.witch_teaching_when_applicable)
        }
        unsupported => return Err(ReconcilerError::UnsupportedCondition(unsupported.into())),
    };

    Ok(match condition.operator {
        domain::ConditionOperator::Equals => actual == condition.value,
        domain::ConditionOperator::NotEquals => match (&actual, &condition.value) {
            (domain::ConditionValue::String(actual), domain::ConditionValue::String(expected)) => {
                actual != expected
            }
            (domain::ConditionValue::Number(actual), domain::ConditionValue::Number(expected)) => {
                actual != expected
            }
            (
                domain::ConditionValue::Boolean(actual),
                domain::ConditionValue::Boolean(expected),
            ) => actual != expected,
            _ => false,
        },
        domain::ConditionOperator::Includes => match (&actual, &condition.value) {
            (domain::ConditionValue::String(actual), domain::ConditionValue::String(expected)) => {
                actual.contains(expected)
            }
            _ => false,
        },
        domain::ConditionOperator::Gte => compare_numbers(&actual, &condition.value, |a, b| a >= b),
        domain::ConditionOperator::Lte => compare_numbers(&actual, &condition.value, |a, b| a <= b),
    })
}

fn enum_condition_value(value: impl serde::Serialize) -> domain::ConditionValue {
    let value = serde_json::to_value(value).expect("列挙値をJSON化できること");
    domain::ConditionValue::String(value.as_str().expect("列挙値が文字列であること").to_owned())
}

fn compare_numbers(
    actual: &domain::ConditionValue,
    expected: &domain::ConditionValue,
    compare: impl FnOnce(f64, f64) -> bool,
) -> bool {
    match (actual, expected) {
        (domain::ConditionValue::Number(actual), domain::ConditionValue::Number(expected)) => {
            compare(*actual, *expected)
        }
        _ => false,
    }
}

fn claim_conflicts_with_variant(
    claim: &domain::ResearchClaim,
    variant: &domain::ResearchBuildVariant,
) -> bool {
    match &claim.normalized_value {
        domain::NormalizedClaimValue::ArtifactPlan { value } => value != &variant.artifact_plan,
        domain::NormalizedClaimValue::MainStatPackage { value } => {
            value != &variant.main_stat_package
        }
        domain::NormalizedClaimValue::SubstatPriority { value } => {
            value != &variant.main_stat_package.substat_priority
        }
        domain::NormalizedClaimValue::TargetStat { .. }
        | domain::NormalizedClaimValue::Role { .. }
        | domain::NormalizedClaimValue::TeamInteraction { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL_ONE: &str = "https://wikiwiki.jp/genshinwiki/one";
    const URL_TWO: &str = "https://game8.jp/genshin/two";
    const URL_THREE: &str = "https://wiki.hoyolab.com/pc/genshin/three";

    fn artifact_plan(set_id: &str) -> domain::ArtifactPlan {
        domain::ArtifactPlan::FourPiece {
            set_id: set_id.to_owned(),
        }
    }

    fn package(sands: &str) -> domain::MainStatPackage {
        domain::MainStatPackage {
            id: "package-1".to_owned(),
            sands: sands.to_owned(),
            goblet: "元素ダメージ".to_owned(),
            circlet: "会心率".to_owned(),
            conditions: Vec::new(),
            substat_priority: vec![domain::StatPriority {
                stat: "会心率".to_owned(),
                rank: 1,
            }],
            target_stats: Vec::new(),
        }
    }

    fn claim(
        claim_type: domain::EvidenceClaimType,
        normalized_value: domain::NormalizedClaimValue,
        source_url: &str,
    ) -> domain::ResearchClaim {
        domain::ResearchClaim {
            claim_type,
            normalized_value,
            conditions: vec![domain::BuildCondition {
                field: "constellation".to_owned(),
                operator: domain::ConditionOperator::Gte,
                value: domain::ConditionValue::Number(1.0),
                description: "検証条件".to_owned(),
            }],
            evidence: domain::ResearchEvidence {
                source_url: source_url.to_owned(),
                evidence_excerpt: Some("抜粋".to_owned()),
                evidence_summary: "要約".to_owned(),
                locator: Some(domain::ResearchLocator {
                    heading: Some("見出し".to_owned()),
                    section: Some("節".to_owned()),
                    text_fragment: Some("断片".to_owned()),
                }),
            },
        }
    }

    fn output_with_claims(claims: Vec<domain::ResearchClaim>) -> domain::CharacterResearchOutput {
        let mut source_urls = HashSet::new();
        for item in &claims {
            source_urls.insert(item.evidence.source_url.clone());
        }
        let sources = source_urls
            .into_iter()
            .map(|source_url| domain::ResearchSourcePage {
                source_url,
                title: "参照ページ".to_owned(),
                publisher: "出版社".to_owned(),
                game_version: "7.0".to_owned(),
                updated_at: None,
            })
            .collect();
        domain::CharacterResearchOutput {
            schema_version: domain::ResearchSchemaVersion::V1,
            character_id: "char-a".to_owned(),
            sources,
            variants: vec![domain::ResearchBuildVariant {
                id: "variant-a".to_owned(),
                artifact_plan: artifact_plan("set-a"),
                main_stat_package: package("攻撃力%"),
                conditions: Vec::new(),
                team_buff_keys: vec!["buff-a".to_owned()],
                claims,
            }],
            warnings: Vec::new(),
        }
    }

    fn required_claims(url: &str) -> Vec<domain::ResearchClaim> {
        let plan = artifact_plan("set-a");
        let main = package("攻撃力%");
        vec![
            claim(
                domain::EvidenceClaimType::ArtifactPlan,
                domain::NormalizedClaimValue::ArtifactPlan { value: plan },
                url,
            ),
            claim(
                domain::EvidenceClaimType::MainStatPackage,
                domain::NormalizedClaimValue::MainStatPackage { value: main },
                url,
            ),
            claim(
                domain::EvidenceClaimType::SubstatPriority,
                domain::NormalizedClaimValue::SubstatPriority {
                    value: vec![domain::StatPriority {
                        stat: "会心率".to_owned(),
                        rank: 1,
                    }],
                },
                url,
            ),
        ]
    }

    fn page(
        source_url: &str,
        source_page_id: &str,
        verification: domain::EvidenceVerification,
        source_family: &str,
    ) -> VerifiedSourcePage {
        VerifiedSourcePage {
            source_url: source_url.to_owned(),
            source_page_id: source_page_id.to_owned(),
            content_hash: Some("hash-1".to_owned()),
            verification,
            source_family: source_family.to_owned(),
            is_direct_content_page: true,
        }
    }

    fn analysis_input() -> domain::AnalysisInput {
        serde_json::from_value(serde_json::json!({
            "partyId": "party-1",
            "partyName": "検証編成",
            "gameVersion": "7.0",
            "members": [
                member(0, "char-a", 1),
                member(1, "char-b", 0),
                member(2, "char-c", 0),
                member(3, "char-d", 0)
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
                "catalogVersion": "catalog-v2",
                "sourcePolicyVersion": "source-v1",
                "promptVersion": "prompt-v1",
                "schemaVersion": "character-research-v1",
                "reconcilerVersion": "reconciler-v1",
                "solverVersion": "solver-v1"
            }
        }))
        .unwrap()
    }

    fn member(slot: u8, character_id: &str, constellation: u8) -> serde_json::Value {
        serde_json::json!({
            "slotIndex": slot,
            "characterId": character_id,
            "weaponId": format!("weapon-{slot}"),
            "refinement": 1,
            "constellation": constellation,
            "intent": {
                "role": "auto",
                "reactionOwnership": "unknown",
                "energyPriority": "balanced",
                "survivabilityPriority": "normal"
            }
        })
    }

    #[test]
    fn 正常変換でホスト値と正規化値を保持する() {
        let output = output_with_claims(required_claims(URL_ONE));
        let result = reconcile_character_research(
            &output,
            &[page(
                URL_ONE,
                "page-1",
                domain::EvidenceVerification::HostExactMatch,
                "wiki",
            )],
            &analysis_input(),
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].character_id, "char-a");
        assert_eq!(result[0].source_family_count, 1);
        assert_eq!(result[0].conflict_penalty, 0);
        assert_eq!(
            result[0].evidence_claims[0].evidence.source_page_id,
            "page-1"
        );
        assert_eq!(
            result[0].evidence_claims[0].evidence.verification,
            domain::EvidenceVerification::HostExactMatch
        );
        assert_eq!(
            result[0].evidence_claims[0].evidence_grade,
            domain::EvidenceGrade::A
        );
        assert_eq!(
            result[0].evidence_claims[0].conditions,
            output.variants[0].claims[0].conditions
        );
    }

    #[test]
    fn 同じsource_familyは一つとして数える() {
        let mut claims = required_claims(URL_ONE);
        claims.push(claim(
            domain::EvidenceClaimType::ArtifactPlan,
            domain::NormalizedClaimValue::ArtifactPlan {
                value: artifact_plan("set-a"),
            },
            URL_TWO,
        ));
        claims.push(claim(
            domain::EvidenceClaimType::ArtifactPlan,
            domain::NormalizedClaimValue::ArtifactPlan {
                value: artifact_plan("set-a"),
            },
            URL_THREE,
        ));
        let output = output_with_claims(claims);
        let result = reconcile_character_research(
            &output,
            &[
                page(
                    URL_ONE,
                    "page-1",
                    domain::EvidenceVerification::HostExactMatch,
                    "wiki",
                ),
                page(
                    URL_TWO,
                    "page-2",
                    domain::EvidenceVerification::HostFuzzyMatch,
                    "wiki",
                ),
                page(
                    URL_THREE,
                    "page-3",
                    domain::EvidenceVerification::HostExactMatch,
                    "guide",
                ),
            ],
            &analysis_input(),
        )
        .unwrap();

        assert_eq!(result[0].source_family_count, 2);
    }

    #[test]
    fn 検証方式ごとに証拠グレードを割り当てる() {
        let cases = [
            (
                domain::EvidenceVerification::HostExactMatch,
                domain::EvidenceGrade::A,
            ),
            (
                domain::EvidenceVerification::HostFuzzyMatch,
                domain::EvidenceGrade::B,
            ),
            (
                domain::EvidenceVerification::UrlEventOnly,
                domain::EvidenceGrade::C,
            ),
            (
                domain::EvidenceVerification::Unverified,
                domain::EvidenceGrade::C,
            ),
        ];
        for (verification, expected_grade) in cases {
            let output = output_with_claims(required_claims(URL_ONE));
            let result = reconcile_character_research(
                &output,
                &[page(URL_ONE, "page-1", verification, "wiki")],
                &analysis_input(),
            );
            if expected_grade == domain::EvidenceGrade::C {
                assert!(matches!(
                    result,
                    Err(ReconcilerError::InsufficientVerifiedEvidence(_))
                ));
            } else {
                assert!(
                    result.unwrap()[0]
                        .evidence_claims
                        .iter()
                        .all(|claim| claim.evidence_grade == expected_grade)
                );
            }
        }
    }

    #[test]
    fn 候補本体と三種類のclaim差分を矛盾として拒否する() {
        let mut claims = required_claims(URL_ONE);
        claims[0].normalized_value = domain::NormalizedClaimValue::ArtifactPlan {
            value: artifact_plan("set-b"),
        };
        claims[1].normalized_value = domain::NormalizedClaimValue::MainStatPackage {
            value: package("元素チャージ効率%"),
        };
        claims[2].normalized_value = domain::NormalizedClaimValue::SubstatPriority {
            value: vec![domain::StatPriority {
                stat: "攻撃力%".to_owned(),
                rank: 1,
            }],
        };
        let output = output_with_claims(claims);
        let result = reconcile_character_research(
            &output,
            &[page(
                URL_ONE,
                "page-1",
                domain::EvidenceVerification::HostExactMatch,
                "wiki",
            )],
            &analysis_input(),
        );

        assert!(matches!(
            result,
            Err(ReconcilerError::ContradictoryVariant(_))
        ));
    }

    #[test]
    fn 未検証urlは日本語エラーになる() {
        let output = output_with_claims(required_claims(URL_ONE));
        let error = reconcile_character_research(&output, &[], &analysis_input()).unwrap_err();

        assert!(error.to_string().contains("検証済みページ"));
        assert!(error.to_string().contains(URL_ONE));
    }

    #[test]
    fn 正規化後に同じ検証済みurlを拒否する() {
        let output = output_with_claims(required_claims(URL_ONE));
        let duplicate_url = format!("{URL_ONE}#fragment");

        let result = reconcile_character_research(
            &output,
            &[
                page(
                    URL_ONE,
                    "page-1",
                    domain::EvidenceVerification::HostExactMatch,
                    "wiki",
                ),
                page(
                    &duplicate_url,
                    "page-2",
                    domain::EvidenceVerification::HostFuzzyMatch,
                    "wiki",
                ),
            ],
            &analysis_input(),
        );

        assert!(matches!(
            result,
            Err(ReconcilerError::DuplicateVerifiedUrl(_))
        ));
    }

    #[test]
    fn 本文未確認ページを必須根拠にできない() {
        let output = output_with_claims(required_claims(URL_ONE));
        let mut verified = page(
            URL_ONE,
            "page-1",
            domain::EvidenceVerification::HostExactMatch,
            "wiki",
        );
        verified.is_direct_content_page = false;

        let result = reconcile_character_research(&output, &[verified], &analysis_input());

        assert!(matches!(
            result,
            Err(ReconcilerError::DirectContentRequired(_))
        ));
    }

    #[test]
    fn claim条件が固定入力と矛盾する候補を拒否する() {
        let output = output_with_claims(required_claims(URL_ONE));
        let mut input = analysis_input();
        input.members[0].constellation = 0;

        let result = reconcile_character_research(
            &output,
            &[page(
                URL_ONE,
                "page-1",
                domain::EvidenceVerification::HostExactMatch,
                "wiki",
            )],
            &input,
        );

        assert!(matches!(
            result,
            Err(ReconcilerError::ContradictoryVariant(_))
        ));
    }

    #[test]
    fn メインステータスpackageの条件も固定入力と照合する() {
        let mut output = output_with_claims(required_claims(URL_ONE));
        let condition = domain::BuildCondition {
            field: "constellation".into(),
            operator: domain::ConditionOperator::Gte,
            value: domain::ConditionValue::Number(2.0),
            description: "2凸以上".into(),
        };
        output.variants[0]
            .main_stat_package
            .conditions
            .push(condition.clone());
        if let domain::NormalizedClaimValue::MainStatPackage { value } =
            &mut output.variants[0].claims[1].normalized_value
        {
            value.conditions.push(condition);
        }

        let result = reconcile_character_research(
            &output,
            &[page(
                URL_ONE,
                "page-1",
                domain::EvidenceVerification::HostExactMatch,
                "wiki",
            )],
            &analysis_input(),
        );

        assert!(matches!(
            result,
            Err(ReconcilerError::ContradictoryVariant(_))
        ));
    }

    #[test]
    fn 条件の型不一致をnot_equalsで通さない() {
        let mut output = output_with_claims(required_claims(URL_ONE));
        output.variants[0].conditions.push(domain::BuildCondition {
            field: "constellation".into(),
            operator: domain::ConditionOperator::NotEquals,
            value: domain::ConditionValue::String("1".into()),
            description: "不正な型".into(),
        });

        let result = reconcile_character_research(
            &output,
            &[page(
                URL_ONE,
                "page-1",
                domain::EvidenceVerification::HostExactMatch,
                "wiki",
            )],
            &analysis_input(),
        );

        assert!(matches!(
            result,
            Err(ReconcilerError::ContradictoryVariant(_))
        ));
    }
}
