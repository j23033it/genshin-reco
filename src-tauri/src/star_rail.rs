use crate::{
    game::GameId,
    on_demand_domain::{ResearchMemberInput, ResearchSource, ResearchedTeamMember},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TunnelSelection {
    FourPiece { set: String },
    TwoPlusTwo { sets: [String; 2] },
}
impl TunnelSelection {
    pub fn names(&self) -> Vec<&str> {
        match self {
            Self::FourPiece { set } => vec![set],
            Self::TwoPlusTwo { sets } => sets.iter().map(String::as_str).collect(),
        }
    }
    pub fn normalized(&self) -> Self {
        match self {
            Self::FourPiece { set } => Self::FourPiece {
                set: set.trim().to_owned(),
            },
            Self::TwoPlusTwo { sets } => {
                let mut sets = sets.clone().map(|name| name.trim().to_owned());
                sets.sort();
                Self::TwoPlusTwo { sets }
            }
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::FourPiece { set } => format!("{set}（4セット）"),
            Self::TwoPlusTwo { sets } => format!("{}（2セット）＋{}（2セット）", sets[0], sets[1]),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelicInput {
    pub tunnel: Option<TunnelSelection>,
    pub ornament: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetEvidence {
    pub set: String,
    pub reason: String,
    // The user's actual stats are not entered. These are requirements, never achievements.
    pub conditions: String,
    #[schemars(length(min = 1, max = 8))]
    pub source_urls: Vec<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarRailBuild {
    #[schemars(range(min = 0, max = 6))]
    pub eidolon: u8,
    pub light_cone: String,
    #[schemars(range(min = 1, max = 5))]
    pub superimposition: u8,
    pub tunnel: TunnelSelection,
    pub ornament: String,
    #[schemars(length(min = 1, max = 2))]
    pub tunnel_evidence: Vec<SetEvidence>,
    pub ornament_evidence: SetEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarRailCharacter {
    pub id: String,
    pub name: String,
    pub element: String,
    pub path: String,
    pub aliases: Vec<String>,
    /// 同時編成できない別形態だけで共有する識別子。
    #[serde(default)]
    pub exclusive_group: Option<String>,
    pub image_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LightCone {
    pub id: String,
    pub name: String,
    pub path: String,
    pub image_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelicSet {
    pub id: String,
    pub name: String,
    pub image_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarRailCatalog {
    pub schema_version: String,
    pub game: GameId,
    pub game_version: String,
    pub catalog_updated_at: String,
    pub characters: Vec<StarRailCharacter>,
    pub light_cones: Vec<LightCone>,
    pub tunnel_relics: Vec<RelicSet>,
    pub ornaments: Vec<RelicSet>,
}

#[tauri::command]
pub fn load_star_rail_catalog() -> Result<StarRailCatalog, String> {
    let catalog: StarRailCatalog =
        serde_json::from_str(include_str!("../../public/data/star-rail/catalog.json"))
            .map_err(|error| error.to_string())?;
    if catalog.schema_version != "star-rail-catalog-v1" || catalog.game != GameId::StarRail {
        return Err("スターレイルカタログの識別が不正です".into());
    }
    for entries in [
        catalog
            .characters
            .iter()
            .map(|entry| (&entry.id, &entry.name))
            .collect::<Vec<_>>(),
        catalog
            .light_cones
            .iter()
            .map(|entry| (&entry.id, &entry.name))
            .collect(),
        catalog
            .tunnel_relics
            .iter()
            .map(|entry| (&entry.id, &entry.name))
            .collect(),
        catalog
            .ornaments
            .iter()
            .map(|entry| (&entry.id, &entry.name))
            .collect(),
    ] {
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for (id, name) in entries {
            if id.trim().is_empty()
                || name.trim().is_empty()
                || !ids.insert(id)
                || !names.insert(name)
            {
                return Err("カタログの名称またはIDが空・重複しています".into());
            }
        }
    }
    Ok(catalog)
}

pub fn validate_input(
    member: &ResearchMemberInput,
    catalog: &StarRailCatalog,
) -> Result<(), String> {
    let character = catalog
        .characters
        .iter()
        .find(|character| character.name == member.name)
        .ok_or_else(|| format!("{}は未登録または形態・運命の確認が必要です", member.name))?;
    if let Some(name) = &member.weapon {
        let cone = catalog
            .light_cones
            .iter()
            .find(|cone| cone.name == *name)
            .ok_or_else(|| format!("光円錐「{name}」は未登録です。再選択してください"))?;
        if cone.path != character.path {
            return Err(format!("{}の運命に光円錐が対応していません", member.name));
        }
    } else if member.refinement.is_some() {
        return Err("光円錐なしで重畳だけ指定できません".into());
    }
    if let Some(relics) = &member.relics {
        if let Some(tunnel) = &relics.tunnel {
            validate_tunnel(tunnel, catalog)?;
        }
        if let Some(ornament) = &relics.ornament {
            validate_set(ornament, &catalog.ornaments)?;
        }
    }
    Ok(())
}

pub fn validate_character_combination<'a>(
    names: impl IntoIterator<Item = &'a str>,
    catalog: &StarRailCatalog,
) -> Result<(), String> {
    let mut groups = HashSet::new();
    for name in names {
        let character = catalog
            .characters
            .iter()
            .find(|character| character.name == name)
            .ok_or_else(|| format!("{name}は未登録または形態・運命の確認が必要です"))?;
        if let Some(group) = character.exclusive_group.as_deref()
            && !groups.insert(group)
        {
            return Err("同時に編成できないキャラクターの別形態が指定されています".into());
        }
    }
    Ok(())
}

fn validate_set(name: &str, entries: &[RelicSet]) -> Result<(), String> {
    if !entries.iter().any(|entry| entry.name == name) {
        return Err(format!(
            "セット「{name}」はこのカテゴリに未登録です。指定を確認・再選択してください"
        ));
    }
    Ok(())
}
fn validate_tunnel(tunnel: &TunnelSelection, catalog: &StarRailCatalog) -> Result<(), String> {
    let names = tunnel.names();
    if names.len() == 2 && names[0] == names[1] {
        return Err("2＋2には異なるセットを選んでください".into());
    }
    for name in names {
        validate_set(name, &catalog.tunnel_relics)?;
    }
    Ok(())
}

pub fn validate_build(
    member: &ResearchedTeamMember,
    input: &ResearchMemberInput,
    sources: &[ResearchSource],
) -> Result<(), String> {
    let catalog = load_star_rail_catalog()?;
    validate_input(input, &catalog)?;
    let build = member
        .star_rail
        .as_ref()
        .ok_or("スターレイルの構造化装備結果がありません")?;
    if build.eidolon > 6
        || !(1..=5).contains(&build.superimposition)
        || build.light_cone != member.weapon
        || member.constellation != format!("{}凸", build.eidolon)
        || input
            .constellation
            .is_some_and(|value| value != build.eidolon)
        || input
            .refinement
            .is_some_and(|value| value != build.superimposition)
    {
        return Err(format!(
            "{}の星魂・光円錐・重畳が指定または表示と一致しません",
            input.name
        ));
    }
    let proposed = ResearchMemberInput {
        weapon: Some(build.light_cone.clone()),
        refinement: Some(build.superimposition),
        relics: Some(RelicInput {
            tunnel: Some(build.tunnel.clone()),
            ornament: Some(build.ornament.clone()),
        }),
        ..input.clone()
    };
    validate_input(&proposed, &catalog)?;
    if let Some(fixed) = &input.relics
        && (fixed
            .tunnel
            .as_ref()
            .is_some_and(|tunnel| tunnel.normalized() != build.tunnel.normalized())
            || fixed
                .ornament
                .as_ref()
                .is_some_and(|name| *name != build.ornament))
    {
        return Err(format!("{}の固定遺物が変更されています", input.name));
    }
    let names = build.tunnel.names();
    if build.tunnel_evidence.len() != names.len() || build.ornament_evidence.set != build.ornament {
        return Err("装備と根拠の対応が不正です".into());
    }
    let evidence_names = build
        .tunnel_evidence
        .iter()
        .map(|e| e.set.as_str())
        .collect::<HashSet<_>>();
    if evidence_names != names.into_iter().collect::<HashSet<_>>() {
        return Err("トンネル遺物の根拠が不足しています".into());
    }
    let allowed = sources
        .iter()
        .map(|s| crate::source_policy::normalize_source_url_for(GameId::StarRail, &s.url))
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| e.to_string())?;
    for evidence in build
        .tunnel_evidence
        .iter()
        .chain(std::iter::once(&build.ornament_evidence))
    {
        if evidence.reason.trim().is_empty()
            || evidence.conditions.trim().is_empty()
            || evidence.source_urls.is_empty()
        {
            return Err(format!(
                "{}の採用理由・発動条件・出典が不足しています",
                evidence.set
            ));
        }
        for url in &evidence.source_urls {
            let url = crate::source_policy::normalize_source_url_for(GameId::StarRail, url)
                .map_err(|e| e.to_string())?;
            if !allowed.contains(&url) {
                return Err("装備の根拠が編成の出典一覧にありません".into());
            }
        }
    }
    Ok(())
}

pub fn apply_images(member: &mut ResearchedTeamMember, catalog: &StarRailCatalog) {
    member.image_url = catalog
        .characters
        .iter()
        .find(|c| c.name == member.name)
        .and_then(|c| c.image_url.clone());
    member.weapon_image_url = catalog
        .light_cones
        .iter()
        .find(|c| c.name == member.weapon)
        .and_then(|c| c.image_url.clone());
    member.artifact_image_url = None;
    if let Some(build) = &mut member.star_rail {
        for evidence in &mut build.tunnel_evidence {
            evidence.image_url = catalog
                .tunnel_relics
                .iter()
                .find(|s| s.name == evidence.set)
                .and_then(|s| s.image_url.clone());
        }
        build.ornament_evidence.image_url = catalog
            .ornaments
            .iter()
            .find(|s| s.name == build.ornament)
            .and_then(|s| s.image_url.clone());
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::on_demand_domain::{ResearchIntake, ResearchedTeamDraft};

    pub(crate) fn sample() -> (ResearchIntake, ResearchedTeamDraft) {
        let names = ["ホタル", "ルアン・メェイ", "開拓者・調和", "ギャラガー"];
        let cones = ["とある星神の殞落を記す", "記憶の中の姿", "輪契", "何が真か"];
        let source = "https://game8.jp/houkaistarrail/12345";
        let intake = ResearchIntake {
            game: GameId::StarRail,
            members: names
                .iter()
                .enumerate()
                .map(|(index, name)| ResearchMemberInput {
                    slot_index: index as u8,
                    name: (*name).into(),
                    weapon: Some(cones[index].into()),
                    constellation: Some(0),
                    refinement: Some(1),
                    relics: None,
                })
                .collect(),
            missing_fields: vec![],
            ready_to_research: true,
        };
        let evidence = |name: &str| SetEvidence {
            set: name.into(),
            reason: "合成テスト用の採用理由".into(),
            conditions: "合成テスト用の必要条件。実測値は未入力。".into(),
            source_urls: vec![source.into()],
            image_url: None,
        };
        let draft: ResearchedTeamDraft = serde_json::from_value(serde_json::json!({
            "game": "star_rail", "teamReasoning": "合成テスト用。支援の分担を説明する。", "title": "テスト", "gameVersion": "test-version",
            "members": names.iter().enumerate().map(|(index, name)| serde_json::json!({
                "slotIndex": index, "id": format!("hsr-{index}"), "name": name, "element": "炎", "role": "合成テスト", "constellation": "0凸",
                "weapon": cones[index], "artifact": "草の穂ガンマン（4セット）", "imageUrl": null, "weaponImageUrl": null, "artifactImageUrl": null,
                "mainStats": "合成テスト", "subStats": "合成テスト", "targetStats": [ {"label": "攻撃力", "value": "合成テスト値", "primary": true, "note": null}, {"label": "速度", "value": "合成テスト値", "primary": false, "note": "未達成の必要条件"} ],
                "starRail": StarRailBuild { eidolon: 0, light_cone: cones[index].into(), superimposition: 1,
                    tunnel: TunnelSelection::FourPiece { set: "草の穂ガンマン".into() }, ornament: "折れた竜骨".into(),
                    tunnel_evidence: vec![evidence("草の穂ガンマン")], ornament_evidence: evidence("折れた竜骨") }
            })).collect::<Vec<_>>(), "sources": [{"title": "合成テスト根拠", "url": source}], "warnings": []
        })).unwrap();
        (intake, draft)
    }

    #[test]
    fn 実在する収録対象と正式な光円錐名を固定指定できる() {
        let (mut intake, _) = sample();
        for (index, name) in ["彦卿", "ルカ", "雲璃", "ブラックスワン"]
            .into_iter()
            .enumerate()
        {
            intake.members[index].name = name.into();
            intake.members[index].weapon = None;
            intake.members[index].refinement = None;
        }
        intake.validate().unwrap();
        for name in ["逃げ場なし", "天傾"] {
            let (mut intake, _) = sample();
            intake.members[0].weapon = Some(name.into());
            intake.validate().unwrap();
        }
        for (slot, name) in [(1, "孤独の癒やし"), (3, "等価交換")] {
            let (mut intake, _) = sample();
            if slot == 1 {
                intake.members[slot].name = "ルカ".into();
            }
            intake.members[slot].weapon = Some(name.into());
            intake.validate().unwrap();
        }
    }

    #[test]
    fn 代表編成のモチーフ光円錐も正式名称と運命で指定できる() {
        let (mut intake, _) = sample();
        intake.members[0].weapon = Some("夢が帰り着く場所".into());
        intake.members[1].weapon = Some("鏡の中の私".into());
        intake.validate().unwrap();
    }

    #[test]
    fn 排他的な別形態だけを入力と結果で拒否する() {
        for names in [
            [
                "開拓者・壊滅",
                "開拓者・存護",
                "開拓者・調和",
                "開拓者・記憶",
            ],
            ["三月なのか", "三月なのか・巡狩", "ホタル", "ギャラガー"],
        ] {
            let (mut intake, mut draft) = sample();
            for (index, name) in names.into_iter().enumerate() {
                intake.members[index].name = name.into();
                intake.members[index].weapon = None;
                intake.members[index].refinement = None;
                draft.members[index].name = name.into();
            }
            assert!(intake.validate().unwrap_err().contains("別形態"));
            assert!(draft.validate().unwrap_err().contains("別形態"));
        }
        let (mut intake, _) = sample();
        for (index, name) in ["丹恒", "丹恒・飲月", "ホタル", "ギャラガー"]
            .into_iter()
            .enumerate()
        {
            intake.members[index].name = name.into();
            intake.members[index].weapon = None;
            intake.members[index].refinement = None;
        }
        intake.validate().unwrap();
    }

    #[test]
    fn 固定なしと片側と両側と複数人同名固定を保持できる() {
        let (mut intake, draft) = sample();
        for mode in 0..4 {
            for member in &mut intake.members {
                member.relics = Some(RelicInput {
                    tunnel: (mode & 1 != 0).then(|| TunnelSelection::FourPiece {
                        set: "草の穂ガンマン".into(),
                    }),
                    ornament: (mode & 2 != 0).then(|| "折れた竜骨".into()),
                });
            }
            assert!(intake.validate().is_ok());
            assert!(draft.validate_for_members(&intake.members).is_ok());
        }
    }

    #[test]
    fn カテゴリ違いと不完全な二足す二と同一セットと運命違いを拒否する() {
        let (mut intake, _) = sample();
        for tunnel in [
            TunnelSelection::FourPiece {
                set: "折れた竜骨".into(),
            },
            TunnelSelection::TwoPlusTwo {
                sets: ["草の穂ガンマン".into(), "".into()],
            },
            TunnelSelection::TwoPlusTwo {
                sets: ["草の穂ガンマン".into(), "草の穂ガンマン".into()],
            },
        ] {
            intake.members[0].relics = Some(RelicInput {
                tunnel: Some(tunnel),
                ornament: None,
            });
            assert!(intake.validate().is_err());
        }
        intake.members[0].relics = Some(RelicInput {
            tunnel: None,
            ornament: Some("草の穂ガンマン".into()),
        });
        assert!(intake.validate().is_err());
        intake.members[0].relics = None;
        intake.members[0].weapon = Some("輪契".into());
        assert!(intake.validate().is_err());
        intake.members[0].weapon = None;
        assert!(intake.validate().is_err());
        intake.members[0].refinement = None;
        assert!(intake.validate().is_ok());
    }

    #[test]
    fn 星魂と重畳と固定セットと根拠の改変をホストで拒否する() {
        let (mut intake, draft) = sample();
        intake.members[0].relics = Some(RelicInput {
            tunnel: Some(TunnelSelection::FourPiece {
                set: "草の穂ガンマン".into(),
            }),
            ornament: Some("折れた竜骨".into()),
        });
        for mutation in 0..6 {
            let mut changed = draft.clone();
            let member = &mut changed.members[0];
            match mutation {
                0 => member.star_rail.as_mut().unwrap().eidolon = 1,
                1 => member.star_rail.as_mut().unwrap().superimposition = 2,
                2 => {
                    member.star_rail.as_mut().unwrap().tunnel = TunnelSelection::FourPiece {
                        set: "夢を弄ぶ時計屋".into(),
                    }
                }
                3 => member.star_rail.as_mut().unwrap().ornament = "生命のウェンワーク".into(),
                4 => member.name = "刃".into(),
                _ => {
                    member
                        .star_rail
                        .as_mut()
                        .unwrap()
                        .ornament_evidence
                        .source_urls = vec!["https://game8.jp/houkaistarrail/99999".into()]
                }
            }
            assert!(
                changed.validate_for_members(&intake.members).is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn 二足す二は両セットの根拠を要求し順序だけでは固定違反にしない() {
        let (mut intake, mut draft) = sample();
        let build = draft.members[0].star_rail.as_mut().unwrap();
        build.tunnel = TunnelSelection::TwoPlusTwo {
            sets: ["草の穂ガンマン".into(), "夢を弄ぶ時計屋".into()],
        };
        let mut second = build.tunnel_evidence[0].clone();
        second.set = "夢を弄ぶ時計屋".into();
        build.tunnel_evidence.push(second);
        intake.members[0].relics = Some(RelicInput {
            tunnel: Some(TunnelSelection::TwoPlusTwo {
                sets: ["夢を弄ぶ時計屋".into(), "草の穂ガンマン".into()],
            }),
            ornament: None,
        });
        assert!(draft.validate_for_members(&intake.members).is_ok());
        draft.members[0]
            .star_rail
            .as_mut()
            .unwrap()
            .tunnel_evidence
            .pop();
        assert!(draft.validate_for_members(&intake.members).is_err());
    }
}
