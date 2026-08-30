use crate::{
    catalog::{ArtifactSet, Catalog},
    domain::{
        ArtifactHalf, ArtifactPlan, BuildVariant, NormalizedClaimValue, validate_analysis_input,
    },
};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CandidateValidationError {
    #[error("分析入力が不正です: {0}")]
    AnalysisInput(String),
    #[error("カタログ照合に失敗しました: {0}")]
    Catalog(String),
}

pub fn validate_analysis_input_against_catalog(
    input: &crate::domain::AnalysisInput,
    catalog: &Catalog,
) -> Result<(), CandidateValidationError> {
    validate_analysis_input(input)
        .map_err(|error| CandidateValidationError::AnalysisInput(error.to_string()))?;
    if input.game_version != catalog.game_version {
        return Err(catalog_error("分析入力とカタログのゲーム版が一致しません"));
    }

    for member in &input.members {
        let character = catalog
            .characters
            .iter()
            .find(|character| character.id == member.character_id)
            .ok_or_else(|| {
                catalog_error(format!(
                    "キャラクターIDがカタログにありません: {}",
                    member.character_id
                ))
            })?;
        let weapon = catalog
            .weapons
            .iter()
            .find(|weapon| weapon.id == member.weapon_id)
            .ok_or_else(|| {
                catalog_error(format!(
                    "武器IDがカタログにありません: {}",
                    member.weapon_id
                ))
            })?;
        if character.weapon_type != weapon.weapon_type {
            return Err(catalog_error(format!(
                "{}と{}の武器種が一致しません",
                character.name, weapon.name
            )));
        }
    }
    Ok(())
}

pub fn validate_build_variants_against_catalog(
    character_id: &str,
    variants: &[BuildVariant],
    catalog: &Catalog,
) -> Result<(), CandidateValidationError> {
    if !catalog
        .characters
        .iter()
        .any(|character| character.id == character_id)
    {
        return Err(catalog_error(format!(
            "キャラクターIDがカタログにありません: {character_id}"
        )));
    }
    if variants.is_empty() || variants.len() > 3 {
        return Err(catalog_error("候補数は1件以上3件以下である必要があります"));
    }

    for variant in variants {
        if variant.character_id != character_id {
            return Err(catalog_error("候補のcharacterIdが調査対象と一致しません"));
        }
        validate_artifact_plan_against_catalog(&variant.artifact_plan, catalog)?;
        validate_team_buff_keys(variant, catalog)?;
        for claim in &variant.evidence_claims {
            if let NormalizedClaimValue::ArtifactPlan { value } = &claim.normalized_value {
                validate_artifact_plan_against_catalog(value, catalog)?;
            }
        }
    }
    Ok(())
}

fn validate_team_buff_keys(
    variant: &BuildVariant,
    catalog: &Catalog,
) -> Result<(), CandidateValidationError> {
    let known_keys = catalog
        .artifact_sets
        .iter()
        .filter_map(|artifact| artifact.team_buff_key.as_deref())
        .collect::<HashSet<_>>();
    let provided = variant
        .team_buff_keys
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    if provided.len() != variant.team_buff_keys.len()
        || provided.iter().any(|key| !known_keys.contains(key))
    {
        return Err(catalog_error(
            "teamBuffKeysに重複またはカタログ外の値があります",
        ));
    }

    let expected = match &variant.artifact_plan {
        ArtifactPlan::FourPiece { set_id } => find_set(catalog, set_id)?
            .team_buff_key
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>(),
        ArtifactPlan::TwoPlusTwo { .. } => HashSet::new(),
    };
    if provided != expected {
        return Err(catalog_error(
            "teamBuffKeysが聖遺物構成のカタログ値と一致しません",
        ));
    }
    Ok(())
}

pub fn validate_artifact_plan_against_catalog(
    plan: &ArtifactPlan,
    catalog: &Catalog,
) -> Result<(), CandidateValidationError> {
    match plan {
        ArtifactPlan::FourPiece { set_id } => {
            let artifact = find_set(catalog, set_id)?;
            if artifact.four_piece_effect.is_none() {
                return Err(catalog_error(format!(
                    "4セット効果が存在しない聖遺物です: {set_id}"
                )));
            }
        }
        ArtifactPlan::TwoPlusTwo { first, second } => {
            let first_ids = resolve_half(first, catalog)?;
            let second_ids = resolve_half(second, catalog)?;
            if !first_ids
                .iter()
                .any(|first_id| second_ids.iter().any(|second_id| second_id != first_id))
            {
                return Err(catalog_error("2+2構成を異なる2セットで成立させられません"));
            }
        }
    }
    Ok(())
}

fn resolve_half<'a>(
    half: &'a ArtifactHalf,
    catalog: &'a Catalog,
) -> Result<Vec<&'a str>, CandidateValidationError> {
    match half {
        ArtifactHalf::ExactSet { set_id } => Ok(vec![find_set(catalog, set_id)?.id.as_str()]),
        ArtifactHalf::EffectGroup { effect_group_id } => {
            let ids = catalog
                .artifact_sets
                .iter()
                .filter(|artifact| artifact.two_piece_effect_group_id == *effect_group_id)
                .map(|artifact| artifact.id.as_str())
                .collect::<Vec<_>>();
            if ids.is_empty() {
                return Err(catalog_error(format!(
                    "2セット効果groupがカタログにありません: {effect_group_id}"
                )));
            }
            Ok(ids)
        }
    }
}

fn find_set<'a>(
    catalog: &'a Catalog,
    set_id: &str,
) -> Result<&'a ArtifactSet, CandidateValidationError> {
    catalog
        .artifact_sets
        .iter()
        .find(|artifact| artifact.id == set_id)
        .ok_or_else(|| catalog_error(format!("聖遺物セットIDがカタログにありません: {set_id}")))
}

fn catalog_error(message: impl Into<String>) -> CandidateValidationError {
    CandidateValidationError::Catalog(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::load_embedded_catalog,
        domain::{ArtifactHalf, MainStatPackage},
    };

    fn variant(plan: ArtifactPlan, team_buff_keys: Vec<String>) -> BuildVariant {
        BuildVariant {
            id: "variant-a".into(),
            character_id: "character-a".into(),
            artifact_plan: plan,
            main_stat_package: MainStatPackage {
                id: "main-a".into(),
                sands: "攻撃力%".into(),
                goblet: "元素ダメージ".into(),
                circlet: "会心率".into(),
                conditions: vec![],
                substat_priority: vec![],
                target_stats: vec![],
            },
            conditions: vec![],
            team_buff_keys,
            evidence_claims: vec![],
            source_family_count: 1,
            conflict_penalty: 0,
        }
    }

    #[test]
    fn カタログに存在する4セットだけを許可する() {
        let catalog = load_embedded_catalog().unwrap();
        let usable = catalog
            .artifact_sets
            .iter()
            .find(|artifact| artifact.four_piece_effect.is_some())
            .unwrap();
        assert!(
            validate_artifact_plan_against_catalog(
                &ArtifactPlan::FourPiece {
                    set_id: usable.id.clone()
                },
                &catalog
            )
            .is_ok()
        );
        assert!(
            validate_artifact_plan_against_catalog(
                &ArtifactPlan::FourPiece {
                    set_id: "unknown-set".into()
                },
                &catalog
            )
            .is_err()
        );
    }

    #[test]
    fn 同じ効果groupから異なる2セットを選べる() {
        let catalog = load_embedded_catalog().unwrap();
        let mut counts = std::collections::HashMap::new();
        for artifact in &catalog.artifact_sets {
            *counts
                .entry(artifact.two_piece_effect_group_id.as_str())
                .or_insert(0_usize) += 1;
        }
        let group = counts
            .into_iter()
            .find_map(|(group, count)| (count >= 2).then_some(group))
            .unwrap();
        let plan = ArtifactPlan::TwoPlusTwo {
            first: ArtifactHalf::EffectGroup {
                effect_group_id: group.into(),
            },
            second: ArtifactHalf::EffectGroup {
                effect_group_id: group.into(),
            },
        };
        assert!(validate_artifact_plan_against_catalog(&plan, &catalog).is_ok());
    }

    #[test]
    fn 一種類しかない効果groupの同士組み合わせを拒否する() {
        let catalog = load_embedded_catalog().unwrap();
        let mut seen = std::collections::HashSet::new();
        let unique_group = catalog
            .artifact_sets
            .iter()
            .find_map(|candidate| {
                let count = catalog
                    .artifact_sets
                    .iter()
                    .filter(|artifact| {
                        artifact.two_piece_effect_group_id == candidate.two_piece_effect_group_id
                    })
                    .count();
                (count == 1 && seen.insert(&candidate.two_piece_effect_group_id))
                    .then_some(candidate.two_piece_effect_group_id.clone())
            })
            .unwrap();
        let half = ArtifactHalf::EffectGroup {
            effect_group_id: unique_group,
        };
        assert!(
            validate_artifact_plan_against_catalog(
                &ArtifactPlan::TwoPlusTwo {
                    first: half.clone(),
                    second: half,
                },
                &catalog
            )
            .is_err()
        );
    }

    #[test]
    fn team_buff_keyは聖遺物カタログから決定する() {
        let catalog = load_embedded_catalog().unwrap();
        let artifact = catalog
            .artifact_sets
            .iter()
            .find(|artifact| {
                artifact.four_piece_effect.is_some() && artifact.team_buff_key.is_some()
            })
            .unwrap();
        let expected = artifact.team_buff_key.clone().unwrap();
        let plan = ArtifactPlan::FourPiece {
            set_id: artifact.id.clone(),
        };

        assert!(validate_team_buff_keys(&variant(plan.clone(), vec![]), &catalog).is_err());
        assert!(
            validate_team_buff_keys(&variant(plan.clone(), vec![expected.clone()]), &catalog)
                .is_ok()
        );
        let unrelated = catalog
            .artifact_sets
            .iter()
            .filter_map(|artifact| artifact.team_buff_key.clone())
            .find(|key| key != &expected)
            .unwrap();
        assert!(
            validate_team_buff_keys(&variant(plan, vec![expected, unrelated]), &catalog).is_err()
        );
    }
}
