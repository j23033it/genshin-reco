use crate::{
    catalog::Catalog,
    database::{Database, DatabaseError, timestamp},
    hashing::sha256_canonical,
    on_demand_domain::{ResearchIntake, ResearchMemberInput, ResearchSource, ResearchedTeamDraft},
    source_policy::{is_direct_content_url, normalize_source_url},
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, future::Future};

const MAX_CACHE_AGE_SECONDS: u64 = 7 * 24 * 60 * 60;
const MAX_SOURCE_HINTS: usize = 12;
const CACHE_ENTITY_TYPE: &str = "team_research";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CachedTeamResearch {
    cache_key: String,
    members: Vec<ResearchMemberInput>,
    pub researched_at: String,
    draft: ResearchedTeamDraft,
}

pub(crate) struct ResearchReuseOutcome {
    pub draft: ResearchedTeamDraft,
    pub cache: Option<CachedTeamResearch>,
    pub reused: bool,
}

fn normalized_members(intake: &ResearchIntake) -> Vec<ResearchMemberInput> {
    let mut members = intake.members.clone();
    for member in &mut members {
        member.name = member.name.trim().to_lowercase();
        member.weapon = member
            .weapon
            .as_ref()
            .map(|name| name.trim().to_lowercase());
    }
    members.sort_by(|left, right| left.name.cmp(&right.name));
    for (index, member) in members.iter_mut().enumerate() {
        member.slot_index = index as u8;
    }
    members
}

pub(crate) fn research_cache_key(
    intake: &ResearchIntake,
    catalog: &Catalog,
    research_revision: &str,
) -> Result<String, String> {
    intake.validate()?;
    sha256_canonical(&serde_json::json!({
        "cacheVersion": 1,
        "members": normalized_members(intake),
        "catalog": catalog,
        "researchRevision": research_revision,
    }))
    .map_err(|error| error.to_string())
}

fn timestamp_seconds(value: &str) -> Option<u64> {
    let (seconds, fraction) = value.strip_suffix('Z')?.split_once('.')?;
    if fraction.len() != 9 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    seconds.parse().ok()
}

fn is_recent(researched_at: &str, now: &str) -> bool {
    timestamp_seconds(now)
        .zip(timestamp_seconds(researched_at))
        .and_then(|(now, researched)| now.checked_sub(researched))
        .is_some_and(|age| age < MAX_CACHE_AGE_SECONDS)
}

impl CachedTeamResearch {
    fn matching_draft(
        &self,
        cache_key: &str,
        intake: &ResearchIntake,
        game_version: &str,
        now: &str,
    ) -> Option<ResearchedTeamDraft> {
        if self.cache_key != cache_key
            || self.members != normalized_members(intake)
            || self.draft.game_version != game_version
            || !is_recent(&self.researched_at, now)
            || self.draft.sources.len() > 32
            || self.draft.warnings.len() > 16
            || self.draft.validate().is_err()
        {
            return None;
        }
        let mut draft = self.draft.clone();
        // 入力の並び順が違っても、同じ4人の結果を現在の順番で表示する。
        draft.members = intake
            .members
            .iter()
            .map(|input| {
                let mut member = self
                    .draft
                    .members
                    .iter()
                    .find(|member| {
                        member.name.trim().to_lowercase() == input.name.trim().to_lowercase()
                    })?
                    .clone();
                if input.weapon.as_ref().is_some_and(|weapon| {
                    weapon.trim().to_lowercase() != member.weapon.trim().to_lowercase()
                }) {
                    return None;
                }
                member.slot_index = input.slot_index;
                Some(member)
            })
            .collect::<Option<Vec<_>>>()?;
        draft.validate().ok()?;
        Some(draft)
    }
}

impl Database {
    fn load_reusable_research(
        &self,
        cache_key: &str,
        intake: &ResearchIntake,
        game_version: &str,
    ) -> Result<Option<(CachedTeamResearch, ResearchedTeamDraft)>, DatabaseError> {
        let payload: Option<String> = self
            .connection()?
            .query_row(
                "SELECT payload_json FROM on_demand_knowledge
             WHERE entity_type = ?1 AND name = ?2 AND game_version = ?3",
                params![CACHE_ENTITY_TYPE, cache_key, game_version],
                |row| row.get(0),
            )
            .optional()?;
        // 壊れた再利用データは通常の調査へ戻し、保存済み編成は消さない。
        Ok(payload
            .and_then(|payload| serde_json::from_str::<CachedTeamResearch>(&payload).ok())
            .and_then(|cache| {
                let draft = cache.matching_draft(cache_key, intake, game_version, &timestamp())?;
                Some((cache, draft))
            }))
    }

    fn load_research_source_hints(
        &self,
        intake: &ResearchIntake,
        game_version: &str,
    ) -> Result<Vec<ResearchSource>, DatabaseError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT sources_json, updated_at FROM on_demand_knowledge
             WHERE entity_type = ?1 AND name = ?2 AND game_version = ?3",
        )?;
        let now = timestamp();
        let mut seen = HashSet::new();
        let mut sources = Vec::new();
        for member in &intake.members {
            for (entity_type, name) in [
                ("character", Some(member.name.as_str())),
                ("weapon", member.weapon.as_deref()),
            ] {
                let Some(name) = name else { continue };
                let entry: Option<(String, String)> = statement
                    .query_row(params![entity_type, name.trim(), game_version], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })
                    .optional()?;
                let Some((payload, researched_at)) = entry else {
                    continue;
                };
                if !is_recent(&researched_at, &now) {
                    continue;
                }
                let Ok(known_sources) = serde_json::from_str::<Vec<ResearchSource>>(&payload)
                else {
                    continue;
                };
                // 他の編成の数値は渡さず、確認し直す個別ページのURLだけを使う。
                for source in known_sources {
                    if source.title.trim().is_empty()
                        || !is_direct_content_url(&source.url).unwrap_or(false)
                    {
                        continue;
                    }
                    let Ok(normalized) = normalize_source_url(&source.url) else {
                        continue;
                    };
                    if seen.insert(normalized) {
                        sources.push(source);
                        if sources.len() == MAX_SOURCE_HINTS {
                            return Ok(sources);
                        }
                    }
                }
            }
        }
        Ok(sources)
    }
}

pub(crate) fn save_research_cache(
    transaction: &Transaction<'_>,
    cache: &CachedTeamResearch,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT INTO on_demand_knowledge (
            knowledge_id, entity_type, name, game_version, payload_json,
            sources_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT(entity_type, name, game_version) DO UPDATE SET
            payload_json = excluded.payload_json,
            sources_json = excluded.sources_json,
            updated_at = excluded.updated_at
         WHERE CAST(excluded.updated_at AS REAL) >= CAST(on_demand_knowledge.updated_at AS REAL)",
        params![
            format!("team-research-{}", cache.cache_key),
            CACHE_ENTITY_TYPE,
            cache.cache_key,
            cache.draft.game_version,
            serde_json::to_string(cache)?,
            serde_json::to_string(&cache.draft.sources)?,
            cache.researched_at,
        ],
    )?;
    Ok(())
}

pub(crate) async fn resolve_team_research<F, Fut>(
    database: &Database,
    intake: &ResearchIntake,
    cache_key: &str,
    game_version: &str,
    refresh: bool,
    research: F,
) -> Result<ResearchReuseOutcome, String>
where
    F: FnOnce(Vec<ResearchSource>) -> Fut,
    Fut: Future<Output = Result<ResearchedTeamDraft, String>>,
{
    if !refresh
        && let Some((cache, draft)) = database
            .load_reusable_research(cache_key, intake, game_version)
            .map_err(|error| error.to_string())?
    {
        return Ok(ResearchReuseOutcome {
            draft,
            cache: Some(cache),
            reused: true,
        });
    }
    let hints = database
        .load_research_source_hints(intake, game_version)
        .map_err(|error| error.to_string())?;
    let draft = research(hints).await?;
    draft.validate()?;
    let cache = (draft.game_version == game_version).then(|| CachedTeamResearch {
        cache_key: cache_key.to_owned(),
        members: normalized_members(intake),
        researched_at: timestamp(),
        draft: draft.clone(),
    });
    Ok(ResearchReuseOutcome {
        draft,
        cache,
        reused: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::load_embedded_catalog,
        on_demand_domain::{
            ResearchConversation, ResearchConversationStatus, ResearchedTeamRecord,
        },
    };

    fn intake() -> ResearchIntake {
        ResearchIntake {
            members: ["アルレッキーノ", "夜蘭", "ベネット", "鍾離"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| ResearchMemberInput {
                    slot_index: index as u8,
                    name: name.into(),
                    weapon: None,
                    constellation: None,
                    refinement: None,
                })
                .collect(),
            missing_fields: Vec::new(),
            ready_to_research: true,
        }
    }

    fn draft(intake: &ResearchIntake) -> ResearchedTeamDraft {
        serde_json::from_value(serde_json::json!({
            "title": "テスト編成", "gameVersion": load_embedded_catalog().unwrap().game_version,
            "members": intake.members.iter().map(|member| serde_json::json!({
                "slotIndex": member.slot_index, "id": format!("member-{}", member.slot_index),
                "name": member.name, "element": "炎", "role": "支援", "constellation": "無凸",
                "imageUrl": null, "weapon": member.weapon.as_deref().unwrap_or("西風長槍"),
                "weaponImageUrl": null, "artifact": "旧貴族のしつけ", "artifactImageUrl": null,
                "mainStats": "HP / HP / HP", "subStats": "HP",
                "targetStats": [
                    {"label": "HP", "value": "30,000以上", "primary": true, "note": null},
                    {"label": "元素チャージ効率", "value": "180%以上", "primary": false, "note": null}
                ]
            })).collect::<Vec<_>>(),
            "sources": [{"title": "根拠", "url": "https://game8.jp/genshin/12345"}],
            "warnings": []
        })).unwrap()
    }

    fn cache(intake: &ResearchIntake, key: &str) -> CachedTeamResearch {
        CachedTeamResearch {
            cache_key: key.into(),
            members: normalized_members(intake),
            researched_at: timestamp(),
            draft: draft(intake),
        }
    }

    fn save_cache(database: &Database, cache: &CachedTeamResearch) {
        let mut connection = database.connection().unwrap();
        let transaction = connection.transaction().unwrap();
        save_research_cache(&transaction, cache).unwrap();
        transaction.commit().unwrap();
    }

    fn saved_record(
        database: &Database,
        intake: &ResearchIntake,
    ) -> (ResearchConversation, ResearchedTeamRecord) {
        let mut conversation = database.create_on_demand_conversation().unwrap();
        conversation.members = intake.members.clone();
        conversation.status = ResearchConversationStatus::Succeeded;
        let draft = draft(intake);
        let record = ResearchedTeamRecord {
            team_id: crate::database::new_id("team"),
            session_id: conversation.session_id.clone(),
            title: draft.title,
            game_version: draft.game_version,
            members: draft.members,
            sources: draft.sources,
            warnings: draft.warnings,
            created_at: timestamp(),
            updated_at: timestamp(),
        };
        conversation.team_id = Some(record.team_id.clone());
        (conversation, record)
    }

    #[test]
    fn 並び順と前後の空白を除いて同じ4人の条件を照合する() {
        let catalog = load_embedded_catalog().unwrap();
        let original = intake();
        let key = research_cache_key(&original, &catalog, "revision-1").unwrap();
        let mut reordered = original.clone();
        reordered.members.reverse();
        for (index, member) in reordered.members.iter_mut().enumerate() {
            member.slot_index = index as u8;
            member.name = format!(" {} ", member.name);
        }
        reordered.missing_fields = vec!["武器".into()];
        assert_eq!(
            key,
            research_cache_key(&reordered, &catalog, "revision-1").unwrap()
        );
        let reused = cache(&original, &key)
            .matching_draft(&key, &reordered, &catalog.game_version, &timestamp())
            .unwrap();
        assert_eq!(reused.members[0].name, "鍾離");
        assert_eq!(reused.members[0].slot_index, 0);
    }

    #[test]
    fn キャラと武器と凸と精錬と版が違う結果を混ぜない() {
        let catalog = load_embedded_catalog().unwrap();
        let original = intake();
        let key = research_cache_key(&original, &catalog, "revision-1").unwrap();
        let cached = cache(&original, &key);
        let mut alternatives = vec![original.clone(); 4];
        alternatives[0].members[0].name = "ディルック".into();
        alternatives[1].members[0].weapon = Some("西風長槍".into());
        alternatives[2].members[0].constellation = Some(0);
        alternatives[3].members[0].refinement = Some(1);
        for changed in alternatives {
            assert_ne!(
                key,
                research_cache_key(&changed, &catalog, "revision-1").unwrap()
            );
            assert!(
                cached
                    .matching_draft(&key, &changed, &catalog.game_version, &timestamp())
                    .is_none()
            );
        }
        assert_ne!(
            key,
            research_cache_key(&original, &catalog, "revision-2").unwrap()
        );
        let mut changed_catalog = catalog.clone();
        changed_catalog.catalog_updated_at.push_str("-changed");
        assert_ne!(
            key,
            research_cache_key(&original, &changed_catalog, "revision-1").unwrap()
        );
        changed_catalog.game_version = "次の版".into();
        assert_ne!(
            key,
            research_cache_key(&original, &changed_catalog, "revision-1").unwrap()
        );
        assert!(
            cached
                .matching_draft(&key, &original, "次の版", &timestamp())
                .is_none()
        );
    }

    #[test]
    fn 期限切れと未来日時と壊れた結果を再利用しない() {
        let intake = intake();
        let mut cached = cache(&intake, "key");
        cached.researched_at = "100.000000000Z".into();
        let version = cached.draft.game_version.clone();
        let before_expiry = format!("{}.000000000Z", 100 + MAX_CACHE_AGE_SECONDS - 1);
        let at_expiry = format!("{}.000000000Z", 100 + MAX_CACHE_AGE_SECONDS);
        assert!(
            cached
                .matching_draft("key", &intake, &version, &before_expiry)
                .is_some()
        );
        assert!(
            cached
                .matching_draft("key", &intake, &version, &at_expiry)
                .is_none()
        );
        assert!(
            cached
                .matching_draft("key", &intake, &version, "99.000000000Z")
                .is_none()
        );
        assert!(!is_recent("壊れた日時", &timestamp()));
        cached.researched_at = timestamp();
        cached.draft.sources[0].url = "https://example.com/".into();
        assert!(
            cached
                .matching_draft("key", &intake, &version, &timestamp())
                .is_none()
        );
    }

    #[tokio::test]
    async fn 一致した成功結果は調査を呼ばず再利用し強制再調査なら呼ぶ() {
        let database = Database::open_in_memory().unwrap();
        let intake = intake();
        let cached = cache(&intake, "key");
        save_cache(&database, &cached);
        let reused = resolve_team_research(
            &database,
            &intake,
            "key",
            &cached.draft.game_version,
            false,
            |_| async { panic!("一致した結果があればWeb調査を呼ばない") },
        )
        .await
        .unwrap();
        assert!(reused.reused);
        assert_eq!(reused.cache.unwrap().researched_at, cached.researched_at);
        let refreshed = resolve_team_research(
            &database,
            &intake,
            "key",
            &cached.draft.game_version,
            true,
            |_| async { Ok(draft(&intake)) },
        )
        .await
        .unwrap();
        assert!(!refreshed.reused);
        assert!(refreshed.cache.is_some());
    }

    #[tokio::test]
    async fn 壊れた再利用データは通常調査へ戻り失敗した調査は追加しない() {
        let database = Database::open_in_memory().unwrap();
        let intake = intake();
        let cached = cache(&intake, "key");
        save_cache(&database, &cached);
        database.connection().unwrap().execute(
            "UPDATE on_demand_knowledge SET payload_json = '{}' WHERE entity_type = 'team_research'", []
        ).unwrap();
        let outcome = resolve_team_research(
            &database,
            &intake,
            "key",
            &cached.draft.game_version,
            false,
            |_| async { Ok(draft(&intake)) },
        )
        .await
        .unwrap();
        assert!(!outcome.reused);
        let failure = resolve_team_research(
            &database,
            &intake,
            "new-key",
            &cached.draft.game_version,
            false,
            |_| async { Err("調査失敗".into()) },
        )
        .await;
        assert!(failure.is_err());
        assert!(
            database
                .load_reusable_research("new-key", &intake, &cached.draft.game_version)
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn 条件変更時は過去の出典だけを渡し本文を確認し直す() {
        let database = Database::open_in_memory().unwrap();
        let original = intake();
        let cached = cache(&original, "key");
        let (conversation, record) = saved_record(&database, &original);
        database
            .save_researched_team_with_cache(&conversation, &record, Some(&cached))
            .unwrap();
        let mut changed = original.clone();
        changed.members[0].constellation = Some(1);
        let changed_input = &changed;
        let outcome = resolve_team_research(
            &database,
            &changed,
            "changed-key",
            &record.game_version,
            false,
            |sources| async move {
                assert_eq!(sources.len(), 1);
                assert_eq!(sources[0].url, "https://game8.jp/genshin/12345");
                Ok(draft(changed_input))
            },
        )
        .await
        .unwrap();
        assert!(!outcome.reused);
        database
            .connection()
            .unwrap()
            .execute(
                "UPDATE on_demand_knowledge SET updated_at = '100.000000000Z'",
                [],
            )
            .unwrap();
        assert!(
            database
                .load_research_source_hints(&changed, &record.game_version)
                .unwrap()
                .is_empty()
        );
        assert!(
            database
                .load_research_source_hints(&changed, "別の版")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn 再利用しても調査日時を延ばさず新しい知識を古い結果で上書きしない() {
        let database = Database::open_in_memory().unwrap();
        let intake = intake();
        let mut cached = cache(&intake, "key");
        cached.researched_at = "100.000000000Z".into();
        let (conversation, record) = saved_record(&database, &intake);
        database
            .save_researched_team_with_cache(&conversation, &record, Some(&cached))
            .unwrap();
        let (another_conversation, newer_record) = saved_record(&database, &intake);
        database
            .save_researched_team(&another_conversation, &newer_record)
            .unwrap();
        database
            .save_researched_team_with_cache(&conversation, &record, Some(&cached))
            .unwrap();
        let connection = database.connection().unwrap();
        let cache_date: String = connection
            .query_row(
                "SELECT updated_at FROM on_demand_knowledge WHERE entity_type = 'team_research'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let knowledge_date: String = connection.query_row(
            "SELECT updated_at FROM on_demand_knowledge WHERE entity_type = 'character' AND name = ?1",
            [&intake.members[0].name], |row| row.get(0)
        ).unwrap();
        assert_eq!(cache_date, cached.researched_at);
        assert_eq!(knowledge_date, newer_record.updated_at);
    }

    #[test]
    fn 再利用データの保存が失敗したら完成編成も保存しない() {
        let database = Database::open_in_memory().unwrap();
        let intake = intake();
        let cached = cache(&intake, "key");
        let (conversation, record) = saved_record(&database, &intake);
        database
            .connection()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_cache BEFORE INSERT ON on_demand_knowledge
             WHEN NEW.entity_type = 'team_research'
             BEGIN SELECT RAISE(ABORT, '保存失敗'); END;",
            )
            .unwrap();
        assert!(
            database
                .save_researched_team_with_cache(&conversation, &record, Some(&cached))
                .is_err()
        );
        assert!(
            database
                .load_researched_team(&record.team_id)
                .unwrap()
                .is_none()
        );
        let restored = database
            .load_on_demand_conversation(&conversation.session_id)
            .unwrap()
            .unwrap();
        assert_eq!(restored.status, ResearchConversationStatus::Collecting);
        let count: i64 = database
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM on_demand_knowledge", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
