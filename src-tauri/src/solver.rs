use crate::domain::{
    BuildVariant, CharacterBuildResolution, EvidenceGrade, ResolutionStatus, TeamBuildResolution,
};
use std::collections::BTreeMap;
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
                .map(|variant| {
                    variant
                        .evidence_claims
                        .iter()
                        .map(|claim| match claim.evidence_grade {
                            EvidenceGrade::A => 3,
                            EvidenceGrade::B => 2,
                            EvidenceGrade::C => 1,
                        })
                        .sum::<u32>()
                })
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
        .ok_or_else(|| SolverError::SearchSpaceExceeded(usize::MAX))?;
    if search_space > MAX_SEARCH_SPACE {
        return Err(SolverError::SearchSpaceExceeded(search_space));
    }

    let mut selections = Vec::with_capacity(search_space);
    let mut indices = vec![0; candidates.len()];
    enumerate_selections(&candidates, 0, &mut indices, &mut selections);

    let best = selections
        .into_iter()
        .min_by(|left, right| compare_selections(left, right))
        .expect("候補は各キャラクター1件以上で検証済み");

    let numeric_tie_with_different_id = best_numeric_tie_has_different_id(&candidates, &best);
    let status = if numeric_tie_with_different_id {
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
            CharacterBuildResolution {
                character_id: selected.character_id.clone(),
                selected_variant_id: Some(selected.id.clone()),
                alternatives: character_candidates
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        (index != selected_index).then_some(candidate.clone())
                    })
                    .collect(),
                reason: if numeric_tie_with_different_id {
                    "数値評価が同点の候補があるため、候補IDを確認して選択してください。".into()
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

fn best_numeric_tie_has_different_id(
    candidates: &[Vec<BuildVariant>],
    best: &EvaluatedSelection<'_>,
) -> bool {
    let mut indices = vec![0; candidates.len()];
    let mut found_different_id = false;
    find_numeric_tie_with_different_id(candidates, 0, &mut indices, best, &mut found_different_id);
    found_different_id
}

fn find_numeric_tie_with_different_id(
    candidates: &[Vec<BuildVariant>],
    depth: usize,
    indices: &mut [usize],
    best: &EvaluatedSelection<'_>,
    found_different_id: &mut bool,
) {
    if *found_different_id {
        return;
    }
    if depth == candidates.len() {
        let variants = indices
            .iter()
            .enumerate()
            .map(|(character_index, candidate_index)| {
                &candidates[character_index][*candidate_index]
            })
            .collect::<Vec<_>>();
        if NumericScore::for_selection(&variants) == best.numeric_score {
            let variant_id_key = variants
                .iter()
                .map(|variant| variant.id.as_str())
                .collect::<Vec<_>>()
                .join("|");
            if variant_id_key != best.variant_id_key {
                *found_different_id = true;
            }
        }
        return;
    }

    for candidate_index in 0..candidates[depth].len() {
        indices[depth] = candidate_index;
        find_numeric_tie_with_different_id(
            candidates,
            depth + 1,
            indices,
            best,
            found_different_id,
        );
    }
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::domain::{
        ArtifactPlan, BuildVariant, EvidenceClaim, EvidenceClaimType, EvidenceVerification,
        MainStatPackage, SourceEvidence,
    };

    fn variant(character_id: &str, id: &str, grade: EvidenceGrade) -> BuildVariant {
        BuildVariant {
            id: id.into(),
            character_id: character_id.into(),
            artifact_plan: ArtifactPlan::FourPiece {
                set_id: "set".into(),
            },
            main_stat_package: MainStatPackage {
                id: "main".into(),
                sands: "攻撃力%".into(),
                goblet: "元素ダメージ".into(),
                circlet: "会心率".into(),
                conditions: vec![],
                substat_priority: vec![],
                target_stats: vec![],
            },
            conditions: vec![],
            team_buff_keys: vec![],
            evidence_claims: vec![EvidenceClaim {
                claim_type: EvidenceClaimType::Role,
                normalized_value: crate::domain::NormalizedClaimValue::Role {
                    value: crate::domain::CharacterBuildIntent {
                        role: crate::domain::BuildIntent::Auto,
                        reaction_ownership: crate::domain::ReactionOwnership::Unknown,
                        energy_priority: crate::domain::EnergyPriority::Balanced,
                        survivability_priority: crate::domain::SurvivabilityPriority::Normal,
                    },
                },
                conditions: vec![],
                evidence: SourceEvidence {
                    source_page_id: "source".into(),
                    evidence_excerpt: None,
                    evidence_summary: "summary".into(),
                    locator: None,
                    verification: EvidenceVerification::HostExactMatch,
                    content_hash: None,
                },
                evidence_grade: grade,
            }],
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
        assert_eq!(
            result.members[0].selected_variant_id.as_deref(),
            Some("v-0-a")
        );
        assert_eq!(result.members[0].alternatives.len(), 1);
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
    }
}
