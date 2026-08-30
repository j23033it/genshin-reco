use crate::domain;
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
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReconcilerError {
    #[error(transparent)]
    Domain(#[from] domain::DomainValidationError),
    #[error("根拠URL「{0}」に対応する検証済みページがありません")]
    UnverifiedUrl(String),
}

/// 旧称を利用する呼び出し側にも同じエラー型を公開する。
pub type ReconcileError = ReconcilerError;

pub fn reconcile_character_research(
    output: &domain::CharacterResearchOutput,
    verified_pages: &[VerifiedSourcePage],
) -> Result<Vec<domain::BuildVariant>, ReconcilerError> {
    domain::validate_character_research_output(output, &output.character_id)?;

    let verified_by_url: HashMap<&str, &VerifiedSourcePage> = verified_pages
        .iter()
        .map(|page| (page.source_url.as_str(), page))
        .collect();

    output
        .variants
        .iter()
        .map(|variant| {
            let mut evidence_claims = Vec::with_capacity(variant.claims.len());
            let mut source_families = HashSet::new();
            let mut conflict_penalty = 0_u32;

            for claim in &variant.claims {
                let page = verified_by_url
                    .get(claim.evidence.source_url.as_str())
                    .ok_or_else(|| {
                        ReconcilerError::UnverifiedUrl(claim.evidence.source_url.clone())
                    })?;
                source_families.insert(page.source_family.as_str());
                if claim_conflicts_with_variant(claim, variant) {
                    conflict_penalty += 1;
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
                    evidence_grade: evidence_grade(page.verification),
                });
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
                conflict_penalty,
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

    const URL_ONE: &str = "https://example.com/one";
    const URL_TWO: &str = "https://example.com/two";
    const URL_THREE: &str = "https://example.com/three";

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
        }
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
            )
            .unwrap();
            assert!(
                result[0]
                    .evidence_claims
                    .iter()
                    .all(|claim| claim.evidence_grade == expected_grade)
            );
        }
    }

    #[test]
    fn 候補本体と三種類のclaim差分を競合penaltyに加算する() {
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
        )
        .unwrap();

        assert_eq!(result[0].conflict_penalty, 3);
    }

    #[test]
    fn 未検証urlは日本語エラーになる() {
        let output = output_with_claims(required_claims(URL_ONE));
        let error = reconcile_character_research(&output, &[]).unwrap_err();

        assert!(error.to_string().contains("検証済みページ"));
        assert!(error.to_string().contains(URL_ONE));
    }
}
