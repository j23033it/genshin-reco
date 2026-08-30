use crate::{
    domain::{
        AnalysisInput, AnalysisStatus, CharacterBuildIntent, CharacterResearchOutput,
        HostGeneratedIdentity, ResolutionStatus, TeamBuildResolution,
    },
    reconciler::VerifiedSourcePage,
    source_policy::normalize_source_url,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    borrow::Borrow,
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: i64 = 3;
pub const BUSY_TIMEOUT_MS: u64 = 5_000;

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS app_meta (
    meta_key TEXT PRIMARY KEY NOT NULL,
    meta_value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS catalog_versions (
    catalog_version TEXT PRIMARY KEY NOT NULL,
    game_version TEXT NOT NULL,
    catalog_hash TEXT NOT NULL,
    loaded_at TEXT NOT NULL,
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json))
);

CREATE TABLE IF NOT EXISTS parties (
    party_id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 40),
    current_result_id TEXT,
    draft_json TEXT NOT NULL CHECK (json_valid(draft_json)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS deleted_parties (
    party_id TEXT PRIMARY KEY NOT NULL,
    deleted_at TEXT NOT NULL,
    FOREIGN KEY (party_id) REFERENCES parties(party_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS party_members (
    party_id TEXT NOT NULL,
    slot_index INTEGER NOT NULL CHECK (slot_index BETWEEN 0 AND 3),
    character_id TEXT,
    weapon_id TEXT,
    refinement INTEGER NOT NULL CHECK (refinement BETWEEN 1 AND 5),
    constellation INTEGER NOT NULL CHECK (constellation BETWEEN 0 AND 6),
    intent_json TEXT NOT NULL CHECK (json_valid(intent_json)),
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    PRIMARY KEY (party_id, slot_index),
    FOREIGN KEY (party_id) REFERENCES parties(party_id) ON DELETE CASCADE,
    CHECK (character_id IS NOT NULL OR weapon_id IS NULL)
);

CREATE TABLE IF NOT EXISTS analysis_runs (
    analysis_run_id TEXT PRIMARY KEY NOT NULL,
    party_id TEXT NOT NULL,
    status TEXT NOT NULL,
    party_composition_hash TEXT,
    analysis_input_hash TEXT,
    evidence_snapshot_hash TEXT,
    result_hash TEXT,
    checked_at TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (party_id) REFERENCES parties(party_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS member_research_jobs (
    job_id TEXT PRIMARY KEY NOT NULL,
    analysis_run_id TEXT NOT NULL,
    slot_index INTEGER NOT NULL CHECK (slot_index BETWEEN 0 AND 3),
    character_id TEXT,
    status TEXT NOT NULL,
    source_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (analysis_run_id) REFERENCES analysis_runs(analysis_run_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS source_pages (
    source_page_id TEXT PRIMARY KEY NOT NULL,
    source_url TEXT NOT NULL,
    normalized_url TEXT NOT NULL,
    title TEXT NOT NULL,
    publisher TEXT NOT NULL,
    game_version TEXT NOT NULL,
    source_family TEXT NOT NULL,
    verification TEXT NOT NULL,
    content_hash TEXT,
    source_json TEXT NOT NULL CHECK (json_valid(source_json)),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS evidence_claims (
    evidence_claim_id TEXT PRIMARY KEY NOT NULL,
    snapshot_id TEXT NOT NULL,
    source_page_id TEXT,
    claim_type TEXT NOT NULL,
    evidence_grade TEXT,
    claim_json TEXT NOT NULL CHECK (json_valid(claim_json)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (snapshot_id) REFERENCES evidence_snapshots(snapshot_id) ON DELETE RESTRICT,
    FOREIGN KEY (source_page_id) REFERENCES source_pages(source_page_id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS evidence_snapshots (
    snapshot_id TEXT PRIMARY KEY NOT NULL,
    analysis_run_id TEXT NOT NULL,
    snapshot_hash TEXT NOT NULL,
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (analysis_run_id) REFERENCES analysis_runs(analysis_run_id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS build_results (
    result_id TEXT PRIMARY KEY NOT NULL,
    analysis_run_id TEXT NOT NULL,
    result_hash TEXT NOT NULL,
    result_json TEXT NOT NULL CHECK (json_valid(result_json)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (analysis_run_id) REFERENCES analysis_runs(analysis_run_id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS user_selections (
    selection_id TEXT PRIMARY KEY NOT NULL,
    result_id TEXT NOT NULL,
    character_id TEXT NOT NULL,
    variant_id TEXT NOT NULL,
    selection_json TEXT NOT NULL CHECK (json_valid(selection_json)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (result_id) REFERENCES build_results(result_id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS app_settings (
    setting_key TEXT PRIMARY KEY NOT NULL,
    setting_value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS character_research_cache (
    cache_id TEXT PRIMARY KEY NOT NULL,
    character_id TEXT NOT NULL,
    game_version TEXT NOT NULL,
    version_key TEXT NOT NULL,
    analysis_input_hash TEXT NOT NULL,
    cache_json TEXT NOT NULL CHECK (json_valid(cache_json)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(character_id, version_key, analysis_input_hash)
);

CREATE INDEX IF NOT EXISTS idx_parties_name ON parties(name);
CREATE INDEX IF NOT EXISTS idx_party_members_character ON party_members(character_id);
CREATE INDEX IF NOT EXISTS idx_analysis_runs_party ON analysis_runs(party_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_member_research_jobs_run ON member_research_jobs(analysis_run_id, slot_index);
CREATE INDEX IF NOT EXISTS idx_source_pages_normalized_url ON source_pages(normalized_url);
CREATE INDEX IF NOT EXISTS idx_evidence_claims_snapshot ON evidence_claims(snapshot_id);
CREATE INDEX IF NOT EXISTS idx_build_results_run ON build_results(analysis_run_id);
CREATE INDEX IF NOT EXISTS idx_character_research_cache_lookup
ON character_research_cache(character_id, game_version, version_key, updated_at DESC);

CREATE TRIGGER IF NOT EXISTS evidence_snapshots_no_update
BEFORE UPDATE ON evidence_snapshots
BEGIN
    SELECT RAISE(ABORT, 'evidence_snapshotsは不変です');
END;

CREATE TRIGGER IF NOT EXISTS evidence_snapshots_no_delete
BEFORE DELETE ON evidence_snapshots
BEGIN
    SELECT RAISE(ABORT, 'evidence_snapshotsは不変です');
END;

CREATE TRIGGER IF NOT EXISTS build_results_no_update
BEFORE UPDATE ON build_results
BEGIN
    SELECT RAISE(ABORT, 'build_resultsは不変です');
END;

CREATE TRIGGER IF NOT EXISTS build_results_no_delete
BEFORE DELETE ON build_results
BEGIN
    SELECT RAISE(ABORT, 'build_resultsは不変です');
END;
"#;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("SQLiteエラー: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSONのシリアライズまたはデシリアライズに失敗しました: {0}")]
    Json(#[from] serde_json::Error),
    #[error("データベースのファイル操作に失敗しました: {0}")]
    Io(#[from] std::io::Error),
    #[error("データベースのロックが壊れています")]
    MutexPoisoned,
    #[error(
        "SQLiteスキーマバージョン{0}は未対応です（現在のバージョンは{CURRENT_SCHEMA_VERSION}です）"
    )]
    UnsupportedSchemaVersion(i64),
    #[error("データベース入力が不正です: {0}")]
    Invalid(String),
    #[error("データベースに必要なデータがありません: {0}")]
    NotFound(String),
}

#[derive(Debug)]
pub struct Database {
    connection: Mutex<Connection>,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartyDraft {
    #[serde(alias = "id")]
    pub party_id: String,
    pub name: String,
    pub members: Vec<PartyMemberDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartyMemberDraft {
    pub slot_index: u8,
    pub character_id: Option<String>,
    pub weapon_id: Option<String>,
    pub refinement: u8,
    pub constellation: u8,
    /// 旧版の保存データを読み込むためだけに保持し、分析入力には使用しない。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<CharacterBuildIntent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PartySummary {
    pub party_id: String,
    pub name: String,
    pub current_result_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRun {
    pub analysis_run_id: String,
    pub party_id: String,
    pub status: AnalysisStatus,
    pub party_composition_hash: Option<String>,
    pub analysis_input_hash: Option<String>,
    pub evidence_snapshot_hash: Option<String>,
    pub result_hash: Option<String>,
    pub checked_at: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuccessfulAnalysisRecord {
    pub identity: HostGeneratedIdentity,
    pub evidence_snapshot: Value,
    pub result: TeamBuildResolution,
}

/// 次回分析で再利用する、ホスト検証済みのキャラクター調査情報。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterResearchCacheRecord {
    pub analysis_input: AnalysisInput,
    pub output: CharacterResearchOutput,
    pub verified_pages: Vec<VerifiedSourcePage>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedCharacterResearchCache {
    pub record: CharacterResearchCacheRecord,
    pub exact_input_match: bool,
}

impl PartyDraft {
    pub fn new(
        party_id: impl Into<String>,
        name: impl Into<String>,
        members: Vec<PartyMemberDraft>,
    ) -> Self {
        Self {
            party_id: party_id.into(),
            name: name.into(),
            members,
        }
    }

    pub fn validate(&self) -> Result<(), DatabaseError> {
        if self.party_id.trim().is_empty() {
            return Err(invalid("編成IDは必須です"));
        }
        let name_length = self.name.chars().count();
        if self.name.trim().is_empty() || !(1..=40).contains(&name_length) {
            return Err(invalid("編成名は1〜40文字で指定してください"));
        }
        if self.members.len() != 4 {
            return Err(invalid("編成スロットは4件必要です"));
        }

        let mut characters = HashSet::new();
        let mut traveler_count = 0;
        for (expected_slot, member) in self.members.iter().enumerate() {
            if member.slot_index != expected_slot as u8 {
                return Err(invalid("slotIndexは0から3の並び順と一致する必要があります"));
            }
            if !(1..=5).contains(&member.refinement) {
                return Err(invalid("精錬ランクは1〜5で指定してください"));
            }
            if member.constellation > 6 {
                return Err(invalid("命ノ星座は0〜6で指定してください"));
            }

            if let Some(character_id) = member.character_id.as_deref() {
                if character_id.trim().is_empty() {
                    return Err(invalid("キャラクターIDは空にできません"));
                }
                if !characters.insert(character_id) {
                    return Err(invalid("同一キャラクターを重複編成できません"));
                }
                if character_id.starts_with("traveler-") {
                    traveler_count += 1;
                }
            } else if member.weapon_id.is_some() {
                return Err(invalid(
                    "キャラクター未選択のスロットに武器を指定できません",
                ));
            }
            if member
                .weapon_id
                .as_deref()
                .is_some_and(|weapon_id| weapon_id.trim().is_empty())
            {
                return Err(invalid("武器IDは空文字列にできません"));
            }
        }
        if traveler_count > 1 {
            return Err(invalid("旅人の異なる元素バリアントを同時編成できません"));
        }
        Ok(())
    }
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DatabaseError> {
        let path = path.as_ref().to_path_buf();
        let existed = path.exists();
        let mut connection = Connection::open(&path)?;
        configure_connection(&mut connection)?;
        migrate(&mut connection, Some(&path), existed)?;
        Ok(Self {
            connection: Mutex::new(connection),
            path: Some(path),
        })
    }

    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, DatabaseError> {
        Self::open(path)
    }

    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let mut connection = Connection::open_in_memory()?;
        configure_connection(&mut connection)?;
        migrate(&mut connection, None, false)?;
        Ok(Self {
            connection: Mutex::new(connection),
            path: None,
        })
    }

    pub fn in_memory() -> Result<Self, DatabaseError> {
        Self::open_in_memory()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn connection(&self) -> Result<MutexGuard<'_, Connection>, DatabaseError> {
        self.connection
            .lock()
            .map_err(|_| DatabaseError::MutexPoisoned)
    }

    pub fn save_character_research_cache(
        &self,
        analysis_input_hash: &str,
        version_key: &str,
        record: &CharacterResearchCacheRecord,
    ) -> Result<(), DatabaseError> {
        if analysis_input_hash.trim().is_empty() || version_key.trim().is_empty() {
            return Err(invalid("調査キャッシュの識別子は必須です"));
        }
        if !record
            .analysis_input
            .members
            .iter()
            .any(|member| member.character_id == record.output.character_id)
        {
            return Err(invalid(
                "調査キャッシュのcharacterIdが分析入力に含まれていません",
            ));
        }
        let cache_json = serde_json::to_string(record)?;
        let now = timestamp();
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO character_research_cache (
                cache_id, character_id, game_version, version_key,
                analysis_input_hash, cache_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(character_id, version_key, analysis_input_hash) DO UPDATE SET
                game_version = excluded.game_version,
                cache_json = excluded.cache_json,
                updated_at = excluded.updated_at",
            params![
                new_id("research-cache"),
                record.output.character_id,
                record.analysis_input.game_version,
                version_key,
                analysis_input_hash,
                cache_json,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn load_character_research_cache(
        &self,
        character_id: &str,
        game_version: &str,
        version_key: &str,
        analysis_input_hash: &str,
    ) -> Result<Option<LoadedCharacterResearchCache>, DatabaseError> {
        if [character_id, game_version, version_key, analysis_input_hash]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(invalid("調査キャッシュの検索条件は必須です"));
        }
        let connection = self.connection()?;
        let row = connection
            .query_row(
                "SELECT analysis_input_hash, cache_json
                 FROM character_research_cache
                 WHERE character_id = ?1 AND game_version = ?2 AND version_key = ?3
                 ORDER BY CASE WHEN analysis_input_hash = ?4 THEN 0 ELSE 1 END,
                          updated_at DESC
                 LIMIT 1",
                params![character_id, game_version, version_key, analysis_input_hash],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        row.map(|(cached_input_hash, cache_json)| {
            Ok(LoadedCharacterResearchCache {
                record: serde_json::from_str(&cache_json)?,
                exact_input_match: cached_input_hash == analysis_input_hash,
            })
        })
        .transpose()
    }

    pub fn save_party<D>(&self, draft: D) -> Result<(), DatabaseError>
    where
        D: Borrow<PartyDraft>,
    {
        let draft = draft.borrow();
        draft.validate()?;
        let draft_json = serde_json::to_string(draft)?;
        let now = timestamp();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO parties (party_id, name, draft_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(party_id) DO UPDATE SET
                 name = excluded.name,
                 draft_json = excluded.draft_json,
                 updated_at = excluded.updated_at",
            params![draft.party_id, draft.name, draft_json, now],
        )?;
        transaction.execute(
            "DELETE FROM deleted_parties WHERE party_id = ?1",
            params![draft.party_id],
        )?;
        transaction.execute(
            "DELETE FROM party_members WHERE party_id = ?1",
            params![draft.party_id],
        )?;
        for member in &draft.members {
            let intent_json = serde_json::to_string(&member.intent)?;
            let member_json = serde_json::to_string(member)?;
            transaction.execute(
                "INSERT INTO party_members
                    (party_id, slot_index, character_id, weapon_id, refinement,
                     constellation, intent_json, snapshot_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    draft.party_id,
                    member.slot_index,
                    member.character_id,
                    member.weapon_id,
                    member.refinement,
                    member.constellation,
                    intent_json,
                    member_json,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn load_party(&self, party_id: &str) -> Result<Option<PartyDraft>, DatabaseError> {
        if party_id.trim().is_empty() {
            return Err(invalid("編成IDは必須です"));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let draft = transaction
            .query_row(
                "SELECT draft_json FROM parties
                 WHERE party_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM deleted_parties
                       WHERE deleted_parties.party_id = parties.party_id
                   )",
                params![party_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let draft = draft
            .map(|json| serde_json::from_str(&json).map_err(DatabaseError::from))
            .transpose()?;
        transaction.commit()?;
        Ok(draft)
    }

    pub fn list_parties(&self) -> Result<Vec<PartySummary>, DatabaseError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let mut statement = transaction.prepare(
            "SELECT party_id, name, current_result_id, updated_at
             FROM parties
             WHERE NOT EXISTS (
                 SELECT 1 FROM deleted_parties
                 WHERE deleted_parties.party_id = parties.party_id
             )
             ORDER BY updated_at DESC, party_id ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(PartySummary {
                party_id: row.get(0)?,
                name: row.get(1)?,
                current_result_id: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        let parties = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        transaction.commit()?;
        Ok(parties)
    }

    pub fn list_party_drafts(&self) -> Result<Vec<PartyDraft>, DatabaseError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT draft_json FROM parties
                 WHERE NOT EXISTS (
                     SELECT 1 FROM deleted_parties
                     WHERE deleted_parties.party_id = parties.party_id
                 )
                 ORDER BY updated_at DESC, party_id ASC",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::from)
    }

    pub fn delete_party(&self, party_id: &str) -> Result<(), DatabaseError> {
        if party_id.trim().is_empty() {
            return Err(invalid("編成IDは必須です"));
        }
        let connection = self.connection()?;
        let exists = connection
            .query_row(
                "SELECT 1 FROM parties WHERE party_id = ?1",
                params![party_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(DatabaseError::NotFound(format!("編成: {party_id}")));
        }
        connection.execute(
            "INSERT INTO deleted_parties (party_id, deleted_at)
             VALUES (?1, ?2)
             ON CONFLICT(party_id) DO UPDATE SET deleted_at = excluded.deleted_at",
            params![party_id, timestamp()],
        )?;
        Ok(())
    }

    pub fn begin_analysis_run(
        &self,
        party_id: &str,
        party_composition_hash: &str,
        analysis_input_hash: &str,
    ) -> Result<String, DatabaseError> {
        if party_id.trim().is_empty()
            || party_composition_hash.trim().is_empty()
            || analysis_input_hash.trim().is_empty()
        {
            return Err(invalid("分析実行の編成IDとhashは必須です"));
        }
        let analysis_run_id = new_id("run");
        let now = timestamp();
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO analysis_runs
                (analysis_run_id, party_id, status, party_composition_hash,
                 analysis_input_hash, created_at, updated_at)
             VALUES (?1, ?2, 'queued', ?3, ?4, ?5, ?5)",
            params![
                analysis_run_id,
                party_id,
                party_composition_hash,
                analysis_input_hash,
                now,
            ],
        )?;
        Ok(analysis_run_id)
    }

    pub fn begin_analysis_run_without_hashes(
        &self,
        party_id: &str,
    ) -> Result<String, DatabaseError> {
        self.begin_analysis_run(party_id, "unknown", "unknown")
    }

    pub fn create_member_research_jobs(
        &self,
        analysis_run_id: &str,
        members: &[crate::domain::PartyMemberInput; 4],
    ) -> Result<Vec<String>, DatabaseError> {
        if analysis_run_id.trim().is_empty() {
            return Err(invalid("分析実行IDは必須です"));
        }
        let now = timestamp();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let mut job_ids = Vec::with_capacity(4);
        for member in members {
            let job_id = new_id("job");
            transaction.execute(
                "INSERT INTO member_research_jobs
                    (job_id, analysis_run_id, slot_index, character_id, status,
                     source_count, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'queued', 0, ?5, ?5)",
                params![
                    job_id,
                    analysis_run_id,
                    member.slot_index,
                    member.character_id,
                    now,
                ],
            )?;
            job_ids.push(job_id);
        }
        transaction.commit()?;
        Ok(job_ids)
    }

    pub fn update_member_research_job(
        &self,
        job_id: &str,
        status: AnalysisStatus,
        source_count: usize,
        error_message: Option<&str>,
    ) -> Result<(), DatabaseError> {
        if job_id.trim().is_empty() {
            return Err(invalid("調査ジョブIDは必須です"));
        }
        let changed = self.connection()?.execute(
            "UPDATE member_research_jobs
             SET status = ?1, source_count = ?2, error_message = ?3, updated_at = ?4
             WHERE job_id = ?5",
            params![
                analysis_status_text(status)?,
                i64::try_from(source_count).map_err(|_| invalid("source件数が大きすぎます"))?,
                error_message,
                timestamp(),
                job_id,
            ],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound(format!("調査ジョブID: {job_id}")));
        }
        Ok(())
    }

    pub fn finish_open_research_jobs(
        &self,
        analysis_run_id: &str,
        status: AnalysisStatus,
        error_message: Option<&str>,
    ) -> Result<(), DatabaseError> {
        if analysis_run_id.trim().is_empty() {
            return Err(invalid("分析実行IDは必須です"));
        }
        self.connection()?.execute(
            "UPDATE member_research_jobs
             SET status = ?1, error_message = ?2, updated_at = ?3
             WHERE analysis_run_id = ?4 AND status != 'succeeded'",
            params![
                analysis_status_text(status)?,
                error_message,
                timestamp(),
                analysis_run_id,
            ],
        )?;
        Ok(())
    }

    pub fn update_run_status(
        &self,
        analysis_run_id: &str,
        status: AnalysisStatus,
        error_message: Option<&str>,
    ) -> Result<(), DatabaseError> {
        if analysis_run_id.trim().is_empty() {
            return Err(invalid("分析実行IDは必須です"));
        }
        let status = analysis_status_text(status)?;
        let now = timestamp();
        let completed_at = matches!(
            status.as_str(),
            "succeeded" | "failed" | "cancelled" | "superseded" | "abandoned"
        )
        .then_some(now.as_str());
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE analysis_runs
             SET status = ?1, error_message = ?2, updated_at = ?3,
                 completed_at = COALESCE(?4, completed_at)
             WHERE analysis_run_id = ?5",
            params![status, error_message, now, completed_at, analysis_run_id],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound(format!(
                "分析実行ID: {analysis_run_id}"
            )));
        }
        Ok(())
    }

    pub fn load_analysis_run(
        &self,
        analysis_run_id: &str,
    ) -> Result<Option<AnalysisRun>, DatabaseError> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                "SELECT analysis_run_id, party_id, status, party_composition_hash,
                        analysis_input_hash, evidence_snapshot_hash, result_hash,
                        checked_at, error_message, created_at, updated_at, completed_at
                 FROM analysis_runs WHERE analysis_run_id = ?1",
                params![analysis_run_id],
                |row| {
                    let status: String = row.get(2)?;
                    Ok(AnalysisRun {
                        analysis_run_id: row.get(0)?,
                        party_id: row.get(1)?,
                        status: parse_analysis_status(&status).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        party_composition_hash: row.get(3)?,
                        analysis_input_hash: row.get(4)?,
                        evidence_snapshot_hash: row.get(5)?,
                        result_hash: row.get(6)?,
                        checked_at: row.get(7)?,
                        error_message: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                        completed_at: row.get(11)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    pub fn persist_successful_result<R>(&self, record: R) -> Result<String, DatabaseError>
    where
        R: Borrow<SuccessfulAnalysisRecord>,
    {
        let record = record.borrow();
        validate_successful_record(record)?;
        let snapshot_json = serde_json::to_string(&record.evidence_snapshot)?;
        let result_json = serde_json::to_string(&record.result)?;
        let snapshot_id = new_id("snapshot");
        let result_id = new_id("result");
        let created_at = record.identity.created_at.clone();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let party_id: String = transaction
            .query_row(
                "SELECT party_id FROM analysis_runs WHERE analysis_run_id = ?1",
                params![record.identity.analysis_run_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                DatabaseError::NotFound(format!("分析実行ID: {}", record.identity.analysis_run_id))
            })?;

        transaction.execute(
            "INSERT INTO evidence_snapshots
                (snapshot_id, analysis_run_id, snapshot_hash, snapshot_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                snapshot_id,
                record.identity.analysis_run_id,
                record.identity.evidence_snapshot_hash,
                snapshot_json,
                created_at,
            ],
        )?;
        insert_snapshot_rows(
            &transaction,
            &snapshot_id,
            &record.evidence_snapshot,
            &created_at,
        )?;
        transaction.execute(
            "INSERT INTO build_results
                (result_id, analysis_run_id, result_hash, result_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                result_id,
                record.identity.analysis_run_id,
                record.identity.result_hash,
                result_json,
                created_at,
            ],
        )?;
        transaction.execute(
            "UPDATE analysis_runs
             SET status = 'succeeded', party_composition_hash = ?1,
                 analysis_input_hash = ?2, evidence_snapshot_hash = ?3,
                 result_hash = ?4, checked_at = ?5, error_message = NULL,
                 updated_at = ?6, completed_at = ?6
             WHERE analysis_run_id = ?7",
            params![
                record.identity.party_composition_hash,
                record.identity.analysis_input_hash,
                record.identity.evidence_snapshot_hash,
                record.identity.result_hash,
                record.identity.checked_at,
                created_at,
                record.identity.analysis_run_id,
            ],
        )?;
        transaction.execute(
            "UPDATE parties SET current_result_id = ?1, updated_at = ?2 WHERE party_id = ?3",
            params![result_id, created_at, party_id],
        )?;
        transaction.commit()?;
        Ok(result_id)
    }

    pub fn load_current_result(
        &self,
        party_id: &str,
    ) -> Result<Option<TeamBuildResolution>, DatabaseError> {
        let connection = self.connection()?;
        let current = connection
            .query_row(
                "SELECT build_results.result_id, build_results.result_json
                 FROM parties
                 JOIN build_results ON build_results.result_id = parties.current_result_id
                 WHERE parties.party_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM deleted_parties
                       WHERE deleted_parties.party_id = parties.party_id
                   )",
                params![party_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((result_id, result_json)) = current else {
            return Ok(None);
        };
        let mut resolution = serde_json::from_str(&result_json)?;
        apply_user_selections(&connection, &result_id, &mut resolution)?;
        Ok(Some(resolution))
    }

    pub fn save_user_selection(
        &self,
        party_id: &str,
        character_id: &str,
        variant_id: &str,
    ) -> Result<TeamBuildResolution, DatabaseError> {
        if party_id.trim().is_empty()
            || character_id.trim().is_empty()
            || variant_id.trim().is_empty()
        {
            return Err(invalid("編成ID・キャラクターID・候補IDは必須です"));
        }
        let connection = self.connection()?;
        let (result_id, result_json): (String, String) = connection
            .query_row(
                "SELECT build_results.result_id, build_results.result_json
                 FROM parties
                 JOIN build_results ON build_results.result_id = parties.current_result_id
                 WHERE parties.party_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM deleted_parties
                       WHERE deleted_parties.party_id = parties.party_id
                   )",
                params![party_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| DatabaseError::NotFound(format!("編成の現在結果: {party_id}")))?;
        let mut resolution: TeamBuildResolution = serde_json::from_str(&result_json)?;
        apply_user_selections(&connection, &result_id, &mut resolution)?;
        let member = resolution
            .members
            .iter()
            .find(|member| member.character_id == character_id)
            .ok_or_else(|| invalid("選択対象キャラクターが現在結果にいません"))?;
        if !member
            .alternatives
            .iter()
            .any(|variant| variant.id == variant_id)
        {
            return Err(invalid("選択候補が現在結果の代替候補にありません"));
        }

        let now = timestamp();
        connection.execute(
            "INSERT INTO user_selections
                (selection_id, result_id, character_id, variant_id, selection_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                new_id("selection"),
                result_id,
                character_id,
                variant_id,
                serde_json::to_string(&serde_json::json!({
                    "characterId": character_id,
                    "variantId": variant_id,
                }))?,
                now,
            ],
        )?;
        apply_user_selection(&mut resolution, character_id, variant_id);
        if resolution
            .members
            .iter()
            .all(|member| member.selected_variant_id.is_some())
        {
            resolution.status = ResolutionStatus::Resolved;
        }
        Ok(resolution)
    }

    pub fn current_result_id(&self, party_id: &str) -> Result<Option<String>, DatabaseError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT current_result_id FROM parties
                 WHERE party_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM deleted_parties
                       WHERE deleted_parties.party_id = parties.party_id
                   )",
                params![party_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(DatabaseError::from)
            .map(|value| value.flatten())
    }
}

fn apply_user_selections(
    connection: &Connection,
    result_id: &str,
    resolution: &mut TeamBuildResolution,
) -> Result<(), DatabaseError> {
    let mut statement = connection.prepare(
        "SELECT character_id, variant_id
         FROM user_selections
         WHERE result_id = ?1
         ORDER BY created_at ASC, selection_id ASC",
    )?;
    let rows = statement.query_map(params![result_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (character_id, variant_id) = row?;
        apply_user_selection(resolution, &character_id, &variant_id);
    }
    if resolution
        .members
        .iter()
        .all(|member| member.selected_variant_id.is_some())
    {
        resolution.status = ResolutionStatus::Resolved;
    }
    Ok(())
}

fn apply_user_selection(
    resolution: &mut TeamBuildResolution,
    character_id: &str,
    variant_id: &str,
) {
    if let Some(member) = resolution.members.iter_mut().find(|member| {
        member.character_id == character_id
            && member
                .alternatives
                .iter()
                .any(|variant| variant.id == variant_id)
    }) {
        member.selected_variant_id = Some(variant_id.to_string());
        member.reason = "ユーザーが同点候補から選択しました。".into();
    }
}

fn configure_connection(connection: &mut Connection) -> Result<(), DatabaseError> {
    connection.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;",
    )?;
    Ok(())
}

fn migrate(
    connection: &mut Connection,
    path: Option<&Path>,
    existed_before_open: bool,
) -> Result<(), DatabaseError> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(DatabaseError::UnsupportedSchemaVersion(version));
    }
    if version < CURRENT_SCHEMA_VERSION {
        if existed_before_open && let Some(path) = path {
            let backup_path = migration_backup_path(path, version);
            connection.backup("main", &backup_path, None)?;
        }
        let transaction = connection.transaction()?;
        transaction.execute_batch(SCHEMA_SQL)?;
        transaction.execute_batch(&format!("PRAGMA user_version = {CURRENT_SCHEMA_VERSION};"))?;
        transaction.commit()?;
    }
    Ok(())
}

fn migration_backup_path(path: &Path, version: i64) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("database.sqlite");
    path.with_file_name(format!("{file_name}.v{version}.bak"))
}

fn insert_snapshot_rows(
    transaction: &Transaction<'_>,
    snapshot_id: &str,
    snapshot: &Value,
    created_at: &str,
) -> Result<(), DatabaseError> {
    let Some(object) = snapshot.as_object() else {
        return Ok(());
    };
    if let Some(source_pages) = object.get("sourcePages").and_then(Value::as_array) {
        for page_value in source_pages {
            let Ok(page) = serde_json::from_value::<VerifiedSourcePage>(page_value.clone()) else {
                continue;
            };
            let source_json = serde_json::to_string(page_value)?;
            let normalized_url = normalize_source_url(&page.source_url)
                .unwrap_or_else(|_| page.source_url.trim().to_owned());
            let verification = serde_json::to_value(page.verification)?
                .as_str()
                .unwrap_or("unverified")
                .to_owned();
            transaction.execute(
                "INSERT OR IGNORE INTO source_pages
                    (source_page_id, source_url, normalized_url, title, publisher,
                     game_version, source_family, verification, content_hash,
                     source_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    page.source_page_id,
                    page.source_url,
                    normalized_url,
                    "",
                    "",
                    "",
                    page.source_family,
                    verification,
                    page.content_hash,
                    source_json,
                    created_at,
                ],
            )?;
        }
    }
    if let Some(claims) = object
        .get("evidenceClaims")
        .or_else(|| object.get("claims"))
        .and_then(Value::as_array)
    {
        for (index, claim) in claims.iter().enumerate() {
            let claim_type = claim
                .get("claimType")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let evidence_grade = claim.get("evidenceGrade").and_then(Value::as_str);
            let source_page_id = claim
                .get("evidence")
                .and_then(|evidence| evidence.get("sourcePageId"))
                .and_then(Value::as_str);
            let source_page_id = match source_page_id {
                Some(source_page_id)
                    if transaction
                        .query_row(
                            "SELECT 1 FROM source_pages WHERE source_page_id = ?1",
                            params![source_page_id],
                            |row| row.get::<_, i64>(0),
                        )
                        .optional()?
                        .is_some() =>
                {
                    Some(source_page_id)
                }
                _ => None,
            };
            transaction.execute(
                "INSERT INTO evidence_claims
                    (evidence_claim_id, snapshot_id, source_page_id, claim_type,
                     evidence_grade, claim_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("{snapshot_id}-claim-{index}"),
                    snapshot_id,
                    source_page_id,
                    claim_type,
                    evidence_grade,
                    serde_json::to_string(claim)?,
                    created_at,
                ],
            )?;
        }
    }
    Ok(())
}

fn validate_successful_record(record: &SuccessfulAnalysisRecord) -> Result<(), DatabaseError> {
    let identity = &record.identity;
    for (label, value) in [
        ("analysisRunId", identity.analysis_run_id.as_str()),
        (
            "partyCompositionHash",
            identity.party_composition_hash.as_str(),
        ),
        ("analysisInputHash", identity.analysis_input_hash.as_str()),
        (
            "evidenceSnapshotHash",
            identity.evidence_snapshot_hash.as_str(),
        ),
        ("resultHash", identity.result_hash.as_str()),
        ("checkedAt", identity.checked_at.as_str()),
        ("createdAt", identity.created_at.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(invalid(format!("{label}は必須です")));
        }
    }
    Ok(())
}

fn analysis_status_text(status: AnalysisStatus) -> Result<String, DatabaseError> {
    serde_json::to_value(status)
        .map_err(DatabaseError::from)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("分析ステータスを文字列化できません"))
}

fn parse_analysis_status(value: &str) -> Result<AnalysisStatus, DatabaseError> {
    serde_json::from_str(&format!("\"{value}\"")).map_err(DatabaseError::from)
}

fn timestamp() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", elapsed.as_secs(), elapsed.subsec_nanos())
}

fn new_id(prefix: &str) -> String {
    static NEXT_ID: OnceLock<Mutex<u64>> = OnceLock::new();
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let sequence = NEXT_ID
        .get_or_init(|| Mutex::new(0))
        .lock()
        .map(|mut value| {
            *value += 1;
            *value
        })
        .unwrap_or_default();
    format!(
        "{prefix}-{}-{}-{sequence}",
        elapsed.as_nanos(),
        std::process::id()
    )
}

fn invalid(message: impl Into<String>) -> DatabaseError {
    DatabaseError::Invalid(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ResolutionStatus;
    use serde_json::json;
    use std::fs;

    fn member(
        slot_index: u8,
        character_id: Option<&str>,
        weapon_id: Option<&str>,
    ) -> PartyMemberDraft {
        PartyMemberDraft {
            slot_index,
            character_id: character_id.map(str::to_owned),
            weapon_id: weapon_id.map(str::to_owned),
            refinement: 1,
            constellation: 0,
            intent: None,
        }
    }

    fn draft(id: &str) -> PartyDraft {
        PartyDraft::new(
            id,
            "検証編成",
            vec![
                member(0, Some("char-a"), Some("weapon-a")),
                member(1, Some("char-b"), Some("weapon-b")),
                member(2, Some("char-c"), Some("weapon-c")),
                member(3, None, None),
            ],
        )
    }

    #[test]
    fn 旧版のintentを読み込めるが新規保存では省略できる() {
        let mut member: PartyMemberDraft = serde_json::from_value(json!({
            "slotIndex": 0,
            "characterId": "char-a",
            "weaponId": "weapon-a",
            "refinement": 1,
            "constellation": 0,
            "intent": {
                "role": "support",
                "reactionOwnership": "unknown",
                "energyPriority": "balanced",
                "survivabilityPriority": "normal"
            }
        }))
        .unwrap();
        assert!(member.intent.is_some());

        member.intent = None;
        let serialized = serde_json::to_value(member).unwrap();
        assert!(serialized.get("intent").is_none());
    }

    fn identity(run_id: &str, suffix: &str) -> HostGeneratedIdentity {
        HostGeneratedIdentity {
            analysis_run_id: run_id.to_owned(),
            party_composition_hash: format!("composition-{suffix}"),
            analysis_input_hash: format!("input-{suffix}"),
            evidence_snapshot_hash: format!("snapshot-{suffix}"),
            result_hash: format!("result-{suffix}"),
            checked_at: format!("checked-{suffix}"),
            created_at: format!("created-{suffix}"),
        }
    }

    fn result() -> TeamBuildResolution {
        TeamBuildResolution {
            status: ResolutionStatus::Resolved,
            members: Vec::new(),
            warnings: vec!["検証".to_owned()],
        }
    }

    fn research_cache_record() -> CharacterResearchCacheRecord {
        CharacterResearchCacheRecord {
            analysis_input: serde_json::from_value(json!({
                "partyId": "party-cache",
                "partyName": "キャッシュ検証",
                "gameVersion": "7.0",
                "members": [
                    { "slotIndex": 0, "characterId": "char-a", "weaponId": "weapon-a", "refinement": 1, "constellation": 0 },
                    { "slotIndex": 1, "characterId": "char-b", "weaponId": "weapon-b", "refinement": 1, "constellation": 0 },
                    { "slotIndex": 2, "characterId": "char-c", "weaponId": "weapon-c", "refinement": 1, "constellation": 0 },
                    { "slotIndex": 3, "characterId": "char-d", "weaponId": "weapon-d", "refinement": 1, "constellation": 0 }
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
            .unwrap(),
            output: serde_json::from_value(json!({
                "schemaVersion": "character-research-v1",
                "characterId": "char-a",
                "sources": [],
                "variants": [],
                "warnings": []
            }))
            .unwrap(),
            verified_pages: Vec::new(),
        }
    }

    fn choice_result() -> TeamBuildResolution {
        serde_json::from_value(serde_json::json!({
            "status": "needs_user_choice",
            "members": [{
                "characterId": "char-a",
                "selectedVariantId": null,
                "alternatives": [{
                    "id": "variant-a",
                    "characterId": "char-a",
                    "artifactPlan": { "type": "four_piece", "setId": "set-a" },
                    "mainStatPackage": {
                        "id": "main-a",
                        "sands": "攻撃力%",
                        "goblet": "元素ダメージ",
                        "circlet": "会心率",
                        "conditions": [],
                        "substatPriority": [],
                        "targetStats": []
                    },
                    "conditions": [],
                    "teamBuffKeys": [],
                    "evidenceClaims": [],
                    "sourceFamilyCount": 1,
                    "conflictPenalty": 0
                }],
                "reason": "同点"
            }],
            "warnings": []
        }))
        .unwrap()
    }

    #[test]
    fn migrationでpragmaとschema_versionを設定する() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        let busy_timeout: i64 = connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert_eq!(busy_timeout, BUSY_TIMEOUT_MS as i64);
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn 検証済み調査キャッシュを完全一致と変更時の両方で読み込む() {
        let database = Database::open_in_memory().unwrap();
        let first = research_cache_record();
        database
            .save_character_research_cache("input-a", "version-a", &first)
            .unwrap();

        let exact = database
            .load_character_research_cache("char-a", "7.0", "version-a", "input-a")
            .unwrap()
            .unwrap();
        assert!(exact.exact_input_match);
        assert_eq!(exact.record, first);

        let mut changed = first;
        changed.analysis_input.members[0].constellation = 1;
        database
            .save_character_research_cache("input-b", "version-a", &changed)
            .unwrap();
        let warm = database
            .load_character_research_cache("char-a", "7.0", "version-a", "input-c")
            .unwrap()
            .unwrap();
        assert!(!warm.exact_input_match);
        assert_eq!(warm.record.analysis_input.members[0].constellation, 1);
    }

    #[test]
    fn file_migrationは移行前backupを作成する() {
        let path = std::env::temp_dir().join(format!("genshin-reco-{}.sqlite", new_id("test")));
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE old_data(value TEXT)", [])
            .unwrap();
        drop(connection);

        let database = Database::open(&path).unwrap();
        let backup = migration_backup_path(&path, 0);
        assert!(backup.exists());
        drop(database);
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(backup);
    }

    #[test]
    fn draftを往復できる() {
        let database = Database::open_in_memory().unwrap();
        let expected = draft("party-1");
        database.save_party(&expected).unwrap();
        assert_eq!(database.load_party("party-1").unwrap(), Some(expected));
    }

    #[test]
    fn 編成31件を上限なく保存できる() {
        let database = Database::open_in_memory().unwrap();
        for index in 0..31 {
            let mut party = draft(&format!("party-{index}"));
            party.name = format!("編成{index}");
            database.save_party(party).unwrap();
        }
        assert_eq!(database.list_parties().unwrap().len(), 31);
    }

    #[test]
    fn 不正な下書きを拒否する() {
        let database = Database::open_in_memory().unwrap();
        let mut invalid_name = draft("party-name");
        invalid_name.name = "".to_owned();
        assert!(database.save_party(invalid_name).is_err());

        let mut duplicate = draft("party-duplicate");
        duplicate.members[1].character_id = Some("char-a".to_owned());
        assert!(database.save_party(duplicate).is_err());

        let mut weapon_without_character = draft("party-weapon");
        weapon_without_character.members[3].weapon_id = Some("weapon-z".to_owned());
        assert!(database.save_party(weapon_without_character).is_err());
    }

    #[test]
    fn 成功結果の保存とcurrent_pointer更新はatomicである() {
        let database = Database::open_in_memory().unwrap();
        database.save_party(draft("party-result")).unwrap();
        let run_id = database
            .begin_analysis_run("party-result", "composition-1", "input-1")
            .unwrap();
        let record = SuccessfulAnalysisRecord {
            identity: identity(&run_id, "one"),
            evidence_snapshot: json!({"sourcePages": [], "evidenceClaims": []}),
            result: result(),
        };
        let result_id = database.persist_successful_result(record).unwrap();
        assert_eq!(
            database.current_result_id("party-result").unwrap(),
            Some(result_id.clone())
        );
        assert_eq!(
            database.load_current_result("party-result").unwrap(),
            Some(result())
        );

        let failed_run = database
            .begin_analysis_run("party-result", "composition-2", "input-2")
            .unwrap();
        database
            .update_run_status(&failed_run, AnalysisStatus::Failed, Some("検証失敗"))
            .unwrap();
        assert_eq!(
            database.current_result_id("party-result").unwrap(),
            Some(result_id)
        );
    }

    #[test]
    fn ユーザー選択を現在結果へ重ねて読み戻す() {
        let database = Database::open_in_memory().unwrap();
        database.save_party(draft("party-choice")).unwrap();
        let run_id = database
            .begin_analysis_run("party-choice", "composition", "input")
            .unwrap();
        database
            .persist_successful_result(SuccessfulAnalysisRecord {
                identity: identity(&run_id, "choice"),
                evidence_snapshot: serde_json::json!({}),
                result: choice_result(),
            })
            .unwrap();

        let selected = database
            .save_user_selection("party-choice", "char-a", "variant-a")
            .unwrap();

        assert_eq!(selected.status, ResolutionStatus::Resolved);
        assert_eq!(
            selected.members[0].selected_variant_id.as_deref(),
            Some("variant-a")
        );
        assert_eq!(
            database
                .load_current_result("party-choice")
                .unwrap()
                .unwrap(),
            selected
        );
        assert!(
            database
                .save_user_selection("party-choice", "char-a", "unknown")
                .is_err()
        );
    }

    #[test]
    fn キャラクター別調査ジョブの状態とsource件数を保存する() {
        let database = Database::open_in_memory().unwrap();
        database.save_party(draft("party-jobs")).unwrap();
        let run_id = database
            .begin_analysis_run("party-jobs", "composition", "input")
            .unwrap();
        let members = std::array::from_fn(|slot_index| crate::domain::PartyMemberInput {
            slot_index: slot_index as u8,
            character_id: format!("char-{slot_index}"),
            weapon_id: format!("weapon-{slot_index}"),
            refinement: 1,
            constellation: 0,
        });

        let jobs = database
            .create_member_research_jobs(&run_id, &members)
            .unwrap();
        database
            .update_member_research_job(&jobs[0], AnalysisStatus::Succeeded, 2, None)
            .unwrap();
        database
            .finish_open_research_jobs(&run_id, AnalysisStatus::Failed, Some("停止"))
            .unwrap();

        let connection = database.connection().unwrap();
        let first: (String, i64) = connection
            .query_row(
                "SELECT status, source_count FROM member_research_jobs WHERE job_id = ?1",
                params![jobs[0]],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let failed_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM member_research_jobs WHERE analysis_run_id = ?1 AND status = 'failed'",
                params![run_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first, ("succeeded".into(), 2));
        assert_eq!(failed_count, 3);
    }

    #[test]
    fn snapshotとresultは不変triggerで保護される() {
        let database = Database::open_in_memory().unwrap();
        database.save_party(draft("party-immutable")).unwrap();
        let run_id = database
            .begin_analysis_run("party-immutable", "composition", "input")
            .unwrap();
        let result_id = database
            .persist_successful_result(SuccessfulAnalysisRecord {
                identity: identity(&run_id, "immutable"),
                evidence_snapshot: json!({}),
                result: result(),
            })
            .unwrap();
        let connection = database.connection().unwrap();
        let snapshot_id: String = connection
            .query_row("SELECT snapshot_id FROM evidence_snapshots", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(
            connection
                .execute(
                    "UPDATE evidence_snapshots SET snapshot_json = '{}' WHERE snapshot_id = ?1",
                    params![snapshot_id]
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "DELETE FROM evidence_snapshots WHERE snapshot_id = ?1",
                    params![snapshot_id]
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "UPDATE build_results SET result_json = '{}' WHERE result_id = ?1",
                    params![result_id]
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "DELETE FROM build_results WHERE result_id = ?1",
                    params![result_id]
                )
                .is_err()
        );
    }
}
