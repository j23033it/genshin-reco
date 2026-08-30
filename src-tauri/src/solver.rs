use crate::domain::{
    BuildVariant, CharacterBuildResolution, EvidenceGrade, ResolutionStatus, TeamBuildResolution,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const MAX_SEARCH_SPACE: usize = 81;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SolverError {
    #[error("solverへの入力が不正です: {0}")]
    Invalid(String),
    #[error("探索候補数が上限81通りを超えています: {0}通り")]
    SearchSpaceExceeded(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NumericScore {
    evidence_grade_total: u32,
    conflict_penalty_total: u32,
    duplicate_buff_count: usize,
    source_family_count_total: usize,
}

impl NumericScore {
    fn for_selection(selection: &[&BuildVariant]) -> Self {
        let mut buff_counts = BTreeMap::<&str, usize>::new();
        for variant in selection {
            for key in &variant.team_buff_keys {
                *buff_counts.entry(key.as_str()).or_default() += 1;
            }
        }

        Self {
            evidence_grade_total: selection
                .iter()
                .map(|variant| evidence_score_by_claim_type(variant))
                .sum(),
            conflict_penalty_total: selection
                .iter()
                .map(|variant| variant.conflict_penalty)
                .sum(),
            duplicate_buff_count: buff_counts
                .values()
                .map(|count| count.saturating_sub(1))
                .sum(),
            source_family_count_total: selection
                .iter()
                .map(|variant| variant.source_family_count)
                .sum(),
        }
    }
}

#[derive(Debug)]
struct EvaluatedSelection<'a> {
    indices: Vec<usize>,
    variants: Vec<&'a BuildVariant>,
    numeric_score: NumericScore,
    variant_id_key: String,
}

/// 4キャラクターの候補を全探索し、決定論的に最良の編成を返す。
pub fn solve_team_builds(
    candidates: Vec<Vec<BuildVariant>>,
) -> Result<TeamBuildResolution, SolverError> {
    validate_candidates(&candidates)?;

    let search_space = candidates
        .iter()
        .try_fold(1usize, |space, character_candidates| {
            space.checked_mul(character_candidates.len())
        })
        .ok_or(SolverError::SearchSpaceExceeded(usize::MAX))?;
    if search_space > MAX_SEARCH_SPACE {
        return Err(SolverError::SearchSpaceExceeded(search_space));
    }

    let mut selections = Vec::with_capacity(search_space);
    let mut indices = vec![0; candidates.len()];
    enumerate_selections(&candidates, 0, &mut indices, &mut selections);

    selections.sort_by(compare_selections);
    let best = selections
        .first()
        .expect("候補は各キャラクター1件以上で検証済み");
    let tied = selections
        .iter()
        .take_while(|selection| selection.numeric_score == best.numeric_score)
        .collect::<Vec<_>>();
    let tied_indices = (0..candidates.len())
        .map(|character_index| {
            tied.iter()
                .map(|selection| selection.indices[character_index])
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    let needs_user_choice = tied_indices.iter().any(|indices| indices.len() > 1);
    let status = if needs_user_choice {
        ResolutionStatus::NeedsUserChoice
    } else {
        ResolutionStatus::Resolved
    };

    let warnings = duplicate_buff_warnings(&best.variants);
    let members = candidates
        .iter()
        .enumerate()
        .map(|(character_index, character_candidates)| {
            let selected_index = best.indices[character_index];
            let selected = &character_candidates[selected_index];
            let ambiguous = tied_indices[character_index].len() > 1;
            CharacterBuildResolution {
                character_id: selected.character_id.clone(),
                selected_variant_id: (!ambiguous).then(|| selected.id.clone()),
                alternatives: if ambiguous {
                    tied_indices[character_index]
                        .iter()
                        .map(|index| character_candidates[*index].clone())
                        .collect()
                } else {
                    character_candidates
                        .iter()
                        .enumerate()
                        .filter_map(|(index, candidate)| {
                            (index != selected_index).then_some(candidate.clone())
                        })
                        .collect()
                },
                reason: if ambiguous {
                    "数値評価が同点のため、自動決定せずユーザー選択を待っています。".into()
                } else if needs_user_choice {
                    "同点候補間で共通して選ばれる候補です。".into()
                } else {
                    "根拠評価・競合・情報源数の比較で最適な候補を選択しました。".into()
                },
            }
        })
        .collect();

    Ok(TeamBuildResolution {
        status,
        members,
        warnings,
    })
}

fn validate_candidates(candidates: &[Vec<BuildVariant>]) -> Result<(), SolverError> {
    if candidates.len() != 4 {
        return Err(SolverError::Invalid(format!(
            "キャラクターは4人必要ですが{}人分です",
            candidates.len()
        )));
    }

    let mut character_ids = BTreeMap::new();
    for (index, character_candidates) in candidates.iter().enumerate() {
        if character_candidates.is_empty() || character_candidates.len() > 3 {
            return Err(SolverError::Invalid(format!(
                "{}人目の候補数は1〜3件である必要があります",
                index + 1
            )));
        }

        let character_id = character_candidates[0].character_id.trim();
        if character_id.is_empty() {
            return Err(SolverError::Invalid(format!(
                "{}人目のcharacterIdが空です",
                index + 1
            )));
        }
        if character_ids.insert(character_id, index).is_some() {
            return Err(SolverError::Invalid(format!(
                "characterId「{}」を編成内で重複できません",
                character_id
            )));
        }

        for candidate in character_candidates {
            if candidate.character_id != character_id {
                return Err(SolverError::Invalid(format!(
                    "{}人目の候補でcharacterIdが統一されていません",
                    index + 1
                )));
            }
            if candidate.id.trim().is_empty() {
                return Err(SolverError::Invalid(format!(
                    "{}人目の候補に空のvariantIdがあります",
                    index + 1
                )));
            }
            validate_required_evidence(candidate)?;
        }
        let unique_ids = character_candidates
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<BTreeSet<_>>();
        if unique_ids.len() != character_candidates.len() {
            return Err(SolverError::Invalid(format!(
                "{}人目のvariantIdが重複しています",
                index + 1
            )));
        }
    }
    Ok(())
}

fn enumerate_selections<'a>(
    candidates: &'a [Vec<BuildVariant>],
    depth: usize,
    indices: &mut [usize],
    selections: &mut Vec<EvaluatedSelection<'a>>,
) {
    if depth == candidates.len() {
        let variants = indices
            .iter()
            .enumerate()
            .map(|(character_index, candidate_index)| {
                &candidates[character_index][*candidate_index]
            })
            .collect::<Vec<_>>();
        let variant_id_key = variants
            .iter()
            .map(|variant| variant.id.as_str())
            .collect::<Vec<_>>()
            .join("|");
        selections.push(EvaluatedSelection {
            indices: indices.to_vec(),
            numeric_score: NumericScore::for_selection(&variants),
            variants,
            variant_id_key,
        });
        return;
    }

    for candidate_index in 0..candidates[depth].len() {
        indices[depth] = candidate_index;
        enumerate_selections(candidates, depth + 1, indices, selections);
    }
}

fn compare_selections(
    left: &EvaluatedSelection<'_>,
    right: &EvaluatedSelection<'_>,
) -> std::cmp::Ordering {
    left.numeric_score
        .evidence_grade_total
        .cmp(&right.numeric_score.evidence_grade_total)
        .reverse()
        .then_with(|| {
            left.numeric_score
                .conflict_penalty_total
                .cmp(&right.numeric_score.conflict_penalty_total)
        })
        .then_with(|| {
            left.numeric_score
                .duplicate_buff_count
                .cmp(&right.numeric_score.duplicate_buff_count)
        })
        .then_with(|| {
            left.numeric_score
                .source_family_count_total
                .cmp(&right.numeric_score.source_family_count_total)
                .reverse()
        })
        .then_with(|| left.variant_id_key.cmp(&right.variant_id_key))
}

fn duplicate_buff_warnings(variants: &[&BuildVariant]) -> Vec<String> {
    let mut buff_counts = BTreeMap::<&str, usize>::new();
    for variant in variants {
        for key in &variant.team_buff_keys {
            *buff_counts.entry(key.as_str()).or_default() += 1;
        }
    }
    buff_counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(key, count)| format!("teamBuffKey「{}」が{}件重複しています。", key, count))
        .collect()
}

fn validate_required_evidence(candidate: &BuildVariant) -> Result<(), SolverError> {
    let trusted = |claim: &crate::domain::EvidenceClaim| {
        claim.evidence_grade != EvidenceGrade::C
            && matches!(
                claim.evidence.verification,
                crate::domain::EvidenceVerification::HostExactMatch
                    | crate::domain::EvidenceVerification::HostFuzzyMatch
            )
            && claim
                .evidence
                .content_hash
                .as_deref()
                .is_some_and(|hash| !hash.trim().is_empty())
            && !claim.evidence.source_page_id.trim().is_empty()
            && !claim.evidence.evidence_summary.trim().is_empty()
    };
    let artifact_supported = candidate.evidence_claims.iter().any(|claim| {
        trusted(claim)
            && claim.claim_type == crate::domain::EvidenceClaimType::ArtifactPlan
            && matches!(
                &claim.normalized_value,
                crate::domain::NormalizedClaimValue::ArtifactPlan { value }
                    if value == &candidate.artifact_plan
            )
    });
    let main_supported = candidate.evidence_claims.iter().any(|claim| {
        trusted(claim)
            && claim.claim_type == crate::domain::EvidenceClaimType::MainStatPackage
            && matches!(
                &claim.normalized_value,
                crate::domain::NormalizedClaimValue::MainStatPackage { value }
                    if value == &candidate.main_stat_package
            )
    });
    let substats_supported = candidate.evidence_claims.iter().any(|claim| {
        trusted(claim)
            && claim.claim_type == crate::domain::EvidenceClaimType::SubstatPriority
            && matches!(
                &claim.normalized_value,
                crate::domain::NormalizedClaimValue::SubstatPriority { value }
                    if value == &candidate.main_stat_package.substat_priority
            )
    });
    if !(artifact_supported && main_supported && substats_supported) {
        return Err(SolverError::Invalid(format!(
            "候補「{}」に本文確認済みの必須根拠がありません",
            candidate.id
        )));
    }
    Ok(())
}

fn evidence_score_by_claim_type(variant: &BuildVariant) -> u32 {
    let mut best_by_type = [0_u32; 6];
    for claim in &variant.evidence_claims {
        let index = match claim.claim_type {
            crate::domain::EvidenceClaimType::ArtifactPlan => 0,
            crate::domain::EvidenceClaimType::MainStatPackage => 1,
            crate::domain::EvidenceClaimType::SubstatPriority => 2,
            crate::domain::EvidenceClaimType::TargetStat => 3,
            crate::domain::EvidenceClaimType::Role => 4,
            crate::domain::EvidenceClaimType::TeamInteraction => 5,
        };
        let score = match claim.evidence_grade {
            EvidenceGrade::A => 3,
            EvidenceGrade::B => 2,
            EvidenceGrade::C => 0,
        };
        best_by_type[index] = best_by_type[index].max(score);
    }
    best_by_type.into_iter().sum()
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::domain::{
        ArtifactPlan, BuildVariant, EvidenceClaim, EvidenceClaimType, EvidenceVerification,
        MainStatPackage, SourceEvidence,
    };

    fn variant(character_id: &str, id: &str, grade: EvidenceGrade) -> BuildVariant {
        let artifact_plan = ArtifactPlan::FourPiece {
            set_id: "set".into(),
        };
        let main_stat_package = MainStatPackage {
            id: "main".into(),
            sands: "攻撃力%".into(),
            goblet: "元素ダメージ".into(),
            circlet: "会心率".into(),
            conditions: vec![],
            substat_priority: vec![crate::domain::StatPriority {
                stat: "会心率".into(),
                rank: 1,
            }],
            target_stats: vec![],
        };
        let evidence = || SourceEvidence {
            source_page_id: "source".into(),
            evidence_excerpt: None,
            evidence_summary: "summary".into(),
            locator: None,
            verification: EvidenceVerification::HostExactMatch,
            content_hash: Some("hash".into()),
        };
        BuildVariant {
            id: id.into(),
            character_id: character_id.into(),
            artifact_plan: artifact_plan.clone(),
            main_stat_package: main_stat_package.clone(),
            conditions: vec![],
            team_buff_keys: vec![],
            evidence_claims: vec![
                EvidenceClaim {
                    claim_type: EvidenceClaimType::ArtifactPlan,
                    normalized_value: crate::domain::NormalizedClaimValue::ArtifactPlan {
                        value: artifact_plan,
                    },
                    conditions: vec![],
                    evidence: evidence(),
                    evidence_grade: grade,
                },
                EvidenceClaim {
                    claim_type: EvidenceClaimType::MainStatPackage,
                    normalized_value: crate::domain::NormalizedClaimValue::MainStatPackage {
                        value: main_stat_package.clone(),
                    },
                    conditions: vec![],
                    evidence: evidence(),
                    evidence_grade: grade,
                },
                EvidenceClaim {
                    claim_type: EvidenceClaimType::SubstatPriority,
                    normalized_value: crate::domain::NormalizedClaimValue::SubstatPriority {
                        value: main_stat_package.substat_priority.clone(),
                    },
                    conditions: vec![],
                    evidence: evidence(),
                    evidence_grade: grade,
                },
            ],
            source_family_count: 1,
            conflict_penalty: 0,
        }
    }

    fn team() -> Vec<Vec<BuildVariant>> {
        (0..4)
            .map(|index| {
                vec![variant(
                    &format!("char-{index}"),
                    &format!("v-{index}"),
                    EvidenceGrade::A,
                )]
            })
            .collect()
    }

    #[test]
    fn 正常系は4人を選択し候補なしのalternativesを返す() {
        let result = solve_team_builds(team()).unwrap();
        assert_eq!(result.status, ResolutionStatus::Resolved);
        assert_eq!(result.members.len(), 4);
        assert!(
            result.members.iter().all(
                |member| member.selected_variant_id.is_some() && member.alternatives.is_empty()
            )
        );
    }

    #[test]
    fn 探索上限81通りを処理できる() {
        let candidates = (0..4)
            .map(|index| {
                (0..3)
                    .map(|candidate_index| {
                        variant(
                            &format!("char-{index}"),
                            &format!("v-{index}-{candidate_index}"),
                            EvidenceGrade::A,
                        )
                    })
                    .collect()
            })
            .collect();
        let result = solve_team_builds(candidates).unwrap();
        assert_eq!(result.members.len(), 4);
    }

    #[test]
    fn 数値評価が同点で候補IDだけ異なる場合は選択要求になる() {
        let mut candidates = team();
        candidates[0][0].id = "v-0-a".into();
        candidates[0].push(variant("char-0", "v-0-z", EvidenceGrade::A));
        let result = solve_team_builds(candidates).unwrap();
        assert_eq!(result.status, ResolutionStatus::NeedsUserChoice);
        assert_eq!(result.members[0].selected_variant_id, None);
        assert_eq!(result.members[0].alternatives.len(), 2);
        assert!(
            result.members[1..]
                .iter()
                .all(|member| member.selected_variant_id.is_some())
        );
        assert!(result.members[0].reason.contains("同点"));
    }

    #[test]
    fn 重複buffは警告しconflictPenaltyを優先する() {
        let mut candidates = team();
        candidates[0][0].team_buff_keys = vec!["atk".into()];
        candidates[1][0].team_buff_keys = vec!["atk".into()];
        candidates[0][0].conflict_penalty = 1;
        let mut better = variant("char-0", "v-0-better", EvidenceGrade::A);
        better.team_buff_keys = vec!["atk".into()];
        better.conflict_penalty = 0;
        candidates[0].push(better);
        let result = solve_team_builds(candidates).unwrap();
        assert_eq!(
            result.members[0].selected_variant_id.as_deref(),
            Some("v-0-better")
        );
        assert_eq!(
            result.warnings,
            vec!["teamBuffKey「atk」が2件重複しています。"]
        );
    }

    #[test]
    fn 同じ入力は常に同じIDを選ぶ() {
        let candidates = team();
        let first = solve_team_builds(candidates.clone()).unwrap();
        let second = solve_team_builds(candidates).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn 入力不正はSolverErrorになる() {
        assert!(matches!(
            solve_team_builds(vec![]),
            Err(SolverError::Invalid(_))
        ));
        let mut candidates = team();
        candidates[0].clear();
        assert!(matches!(
            solve_team_builds(candidates),
            Err(SolverError::Invalid(_))
        ));

        let mut candidates = team();
        candidates[0].push(variant("char-other", "v-other", EvidenceGrade::A));
        assert!(matches!(
            solve_team_builds(candidates),
            Err(SolverError::Invalid(_))
        ));

        let mut candidates = team();
        candidates[0].push(variant("char-0", "v-0", EvidenceGrade::A));
        assert!(matches!(
            solve_team_builds(candidates),
            Err(SolverError::Invalid(_))
        ));
    }

    #[test]
    fn c評価の必須根拠はsolver直接入力でも拒否する() {
        let mut candidates = team();
        for claim in &mut candidates[0][0].evidence_claims {
            claim.evidence_grade = EvidenceGrade::C;
            claim.evidence.verification = EvidenceVerification::UrlEventOnly;
            claim.evidence.content_hash = None;
        }

        assert!(matches!(
            solve_team_builds(candidates),
            Err(SolverError::Invalid(message)) if message.contains("必須根拠")
        ));
    }

    #[test]
    fn 候補本体と異なるclaimはsolver直接入力でも拒否する() {
        let mut candidates = team();
        candidates[0][0].artifact_plan = ArtifactPlan::FourPiece {
            set_id: "different-set".into(),
        };

        assert!(matches!(
            solve_team_builds(candidates),
            Err(SolverError::Invalid(message)) if message.contains("必須根拠")
        ));
    }

    #[test]
    fn 同じclaim種別の複製で評価を水増しできない() {
        let base = variant("char-a", "base", EvidenceGrade::A);
        let mut duplicated = base.clone();
        let duplicate = duplicated.evidence_claims[0].clone();
        duplicated
            .evidence_claims
            .extend([duplicate.clone(), duplicate]);

        assert_eq!(
            evidence_score_by_claim_type(&base),
            evidence_score_by_claim_type(&duplicated)
        );
    }
}
