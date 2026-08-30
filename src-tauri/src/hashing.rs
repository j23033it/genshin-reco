use crate::domain::AnalysisInput;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HashingError {
    #[error("hash対象をJSON化できません: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompositionMember<'a> {
    character_id: &'a str,
    weapon_id: &'a str,
    refinement: u8,
    constellation: u8,
}

pub fn party_composition_hash(input: &AnalysisInput) -> Result<String, HashingError> {
    let mut members = input
        .members
        .iter()
        .map(|member| CompositionMember {
            character_id: &member.character_id,
            weapon_id: &member.weapon_id,
            refinement: member.refinement,
            constellation: member.constellation,
        })
        .collect::<Vec<_>>();
    members.sort_by(|left, right| {
        left.character_id
            .cmp(right.character_id)
            .then(left.weapon_id.cmp(right.weapon_id))
            .then(left.refinement.cmp(&right.refinement))
            .then(left.constellation.cmp(&right.constellation))
    });
    sha256_canonical(&members)
}

pub fn analysis_input_hash(input: &AnalysisInput) -> Result<String, HashingError> {
    let composition_hash = party_composition_hash(input)?;
    let mut members = input
        .members
        .iter()
        .map(|member| CompositionMember {
            character_id: &member.character_id,
            weapon_id: &member.weapon_id,
            refinement: member.refinement,
            constellation: member.constellation,
        })
        .collect::<Vec<_>>();
    members.sort_by(|left, right| left.character_id.cmp(right.character_id));

    sha256_canonical(&json!({
        "partyCompositionHash": composition_hash,
        "members": members,
        "assumptions": &input.assumptions,
        "gameVersion": &input.game_version,
        "versions": &input.versions,
    }))
}

pub fn evidence_snapshot_hash<T: Serialize>(snapshot: &T) -> Result<String, HashingError> {
    sha256_canonical(snapshot)
}

pub fn result_hash(
    analysis_input_hash: &str,
    evidence_snapshot_hash: &str,
    solver_version: &str,
) -> Result<String, HashingError> {
    sha256_canonical(&json!({
        "analysisInputHash": analysis_input_hash,
        "evidenceSnapshotHash": evidence_snapshot_hash,
        "solverVersion": solver_version,
    }))
}

pub fn sha256_canonical<T: Serialize>(value: &T) -> Result<String, HashingError> {
    let value = serde_json::to_value(value)?;
    let canonical = canonicalize(value);
    let bytes = serde_json::to_vec(&canonical)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        Value::Object(values) => {
            let mut entries = values.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, canonicalize(value)))
                    .collect::<Map<_, _>>(),
            )
        }
        scalar => scalar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::AnalysisInput;
    use serde_json::json;

    fn input() -> AnalysisInput {
        serde_json::from_value(json!({
            "partyId": "party-1",
            "partyName": "検証編成",
            "gameVersion": "7.0",
            "members": [
                member(0, "char-a", "weapon-a"),
                member(1, "char-b", "weapon-b"),
                member(2, "char-c", "weapon-c"),
                member(3, "char-d", "weapon-d")
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
        }))
        .unwrap()
    }

    fn member(slot: u8, character: &str, weapon: &str) -> Value {
        json!({
            "slotIndex": slot,
            "characterId": character,
            "weaponId": weapon,
            "refinement": 1,
            "constellation": 0
        })
    }

    #[test]
    fn objectのキー順に依存せず同じhashになる() {
        assert_eq!(
            sha256_canonical(&json!({ "a": 1, "b": 2 })).unwrap(),
            sha256_canonical(&json!({ "b": 2, "a": 1 })).unwrap()
        );
    }

    #[test]
    fn スロット順と編成名はhashへ影響しない() {
        let first = input();
        let mut reordered = first.clone();
        reordered.members.reverse();
        reordered.party_name = "別名".into();
        assert_eq!(
            party_composition_hash(&first).unwrap(),
            party_composition_hash(&reordered).unwrap()
        );
        assert_eq!(
            analysis_input_hash(&first).unwrap(),
            analysis_input_hash(&reordered).unwrap()
        );
    }

    #[test]
    fn 武器変更は両方のhashを変更する() {
        let first = input();
        let mut changed = first.clone();
        changed.members[0].weapon_id = "weapon-z".into();
        assert_ne!(
            party_composition_hash(&first).unwrap(),
            party_composition_hash(&changed).unwrap()
        );
        assert_ne!(
            analysis_input_hash(&first).unwrap(),
            analysis_input_hash(&changed).unwrap()
        );
    }
}
