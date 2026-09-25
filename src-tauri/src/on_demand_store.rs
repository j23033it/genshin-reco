use crate::{
    database::{Database, DatabaseError, new_id, timestamp},
    on_demand_domain::{
        ResearchConversation, ResearchConversationStatus, ResearchedTeamRecord,
        ResearchedTeamSummary,
    },
};
use rusqlite::{OptionalExtension, params};

fn status_text(status: ResearchConversationStatus) -> Result<String, DatabaseError> {
    serde_json::to_value(status)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| DatabaseError::Invalid("会話状態を文字列化できません".into()))
}

impl Database {
    pub fn create_on_demand_conversation(&self) -> Result<ResearchConversation, DatabaseError> {
        let now = timestamp();
        let conversation = ResearchConversation {
            session_id: new_id("research-session"),
            status: ResearchConversationStatus::Collecting,
            messages: Vec::new(),
            members: Vec::new(),
            missing_fields: Vec::new(),
            team_id: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.save_on_demand_conversation(&conversation)?;
        Ok(conversation)
    }

    pub fn save_on_demand_conversation(
        &self,
        conversation: &ResearchConversation,
    ) -> Result<(), DatabaseError> {
        validate_conversation(conversation)?;
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO on_demand_research_sessions (
                session_id, status, conversation_json, team_id, error_message, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id) DO UPDATE SET
                status = excluded.status,
                conversation_json = excluded.conversation_json,
                team_id = excluded.team_id,
                error_message = excluded.error_message,
                updated_at = excluded.updated_at",
            params![
                conversation.session_id,
                status_text(conversation.status)?,
                serde_json::to_string(conversation)?,
                conversation.team_id,
                conversation.error,
                conversation.created_at,
                conversation.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn load_on_demand_conversation(
        &self,
        session_id: &str,
    ) -> Result<Option<ResearchConversation>, DatabaseError> {
        if session_id.trim().is_empty() {
            return Err(DatabaseError::Invalid("会話IDは必須です".into()));
        }
        let connection = self.connection()?;
        let json = connection
            .query_row(
                "SELECT conversation_json FROM on_demand_research_sessions WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|json| serde_json::from_str(&json).map_err(DatabaseError::from))
            .transpose()
    }

    pub fn save_researched_team(
        &self,
        conversation: &ResearchConversation,
        record: &ResearchedTeamRecord,
    ) -> Result<(), DatabaseError> {
        validate_conversation(conversation)?;
        if conversation.session_id != record.session_id
            || conversation.team_id.as_deref() != Some(record.team_id.as_str())
            || conversation.status != ResearchConversationStatus::Succeeded
        {
            return Err(DatabaseError::Invalid(
                "保存する会話と編成結果の識別子または状態が一致しません".into(),
            ));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO researched_teams (
                team_id, session_id, title, result_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(team_id) DO UPDATE SET
                title = excluded.title,
                result_json = excluded.result_json,
                updated_at = excluded.updated_at",
            params![
                record.team_id,
                record.session_id,
                record.title,
                serde_json::to_string(record)?,
                record.created_at,
                record.updated_at,
            ],
        )?;
        transaction.execute(
            "UPDATE on_demand_research_sessions
             SET status = ?2, conversation_json = ?3, team_id = ?4,
                 error_message = NULL, updated_at = ?5
             WHERE session_id = ?1",
            params![
                conversation.session_id,
                status_text(conversation.status)?,
                serde_json::to_string(conversation)?,
                record.team_id,
                conversation.updated_at,
            ],
        )?;

        let sources_json = serde_json::to_string(&record.sources)?;
        for member in &record.members {
            upsert_knowledge(
                &transaction,
                "character",
                &member.name,
                &record.game_version,
                &serde_json::to_string(member)?,
                &sources_json,
                &record.updated_at,
            )?;
            upsert_knowledge(
                &transaction,
                "weapon",
                &member.weapon,
                &record.game_version,
                &serde_json::to_string(&serde_json::json!({
                    "name": member.weapon,
                    "imageUrl": member.weapon_image_url,
                }))?,
                &sources_json,
                &record.updated_at,
            )?;
            upsert_knowledge(
                &transaction,
                "artifact",
                &member.artifact,
                &record.game_version,
                &serde_json::to_string(&serde_json::json!({
                    "name": member.artifact,
                    "imageUrl": member.artifact_image_url,
                }))?,
                &sources_json,
                &record.updated_at,
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_researched_team_summaries(
        &self,
    ) -> Result<Vec<ResearchedTeamSummary>, DatabaseError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT result_json FROM researched_teams ORDER BY updated_at DESC, team_id DESC",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut summaries = Vec::new();
        for row in rows {
            let record: ResearchedTeamRecord = serde_json::from_str(&row?)?;
            summaries.push(ResearchedTeamSummary {
                team_id: record.team_id,
                title: record.title,
                member_names: record
                    .members
                    .iter()
                    .map(|member| member.name.clone())
                    .collect(),
                member_image_urls: record
                    .members
                    .iter()
                    .map(|member| member.image_url.clone())
                    .collect(),
                updated_at: record.updated_at,
            });
        }
        Ok(summaries)
    }

    pub fn load_researched_team(
        &self,
        team_id: &str,
    ) -> Result<Option<ResearchedTeamRecord>, DatabaseError> {
        if team_id.trim().is_empty() {
            return Err(DatabaseError::Invalid("編成IDは必須です".into()));
        }
        let connection = self.connection()?;
        let json = connection
            .query_row(
                "SELECT result_json FROM researched_teams WHERE team_id = ?1",
                params![team_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|json| serde_json::from_str(&json).map_err(DatabaseError::from))
            .transpose()
    }

    #[cfg(test)]
    fn count_on_demand_knowledge(&self) -> Result<i64, DatabaseError> {
        let connection = self.connection()?;
        connection
            .query_row("SELECT COUNT(*) FROM on_demand_knowledge", [], |row| {
                row.get(0)
            })
            .map_err(DatabaseError::from)
    }
}

fn validate_conversation(conversation: &ResearchConversation) -> Result<(), DatabaseError> {
    if conversation.session_id.trim().is_empty() {
        return Err(DatabaseError::Invalid("会話IDは必須です".into()));
    }
    if conversation.messages.len() > 200 {
        return Err(DatabaseError::Invalid(
            "1会話に保存できるメッセージは200件までです".into(),
        ));
    }
    if conversation.members.len() > 4 {
        return Err(DatabaseError::Invalid("調査対象は4人までです".into()));
    }
    Ok(())
}

fn upsert_knowledge(
    transaction: &rusqlite::Transaction<'_>,
    entity_type: &str,
    name: &str,
    game_version: &str,
    payload_json: &str,
    sources_json: &str,
    now: &str,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT INTO on_demand_knowledge (
            knowledge_id, entity_type, name, game_version, payload_json,
            sources_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT(entity_type, name, game_version) DO UPDATE SET
            payload_json = excluded.payload_json,
            sources_json = excluded.sources_json,
            updated_at = excluded.updated_at",
        params![
            new_id("knowledge"),
            entity_type,
            name,
            game_version,
            payload_json,
            sources_json,
            now,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::on_demand_domain::{
        ResearchMessage, ResearchMessageRole, ResearchSource, ResearchedTargetStat,
        ResearchedTeamMember,
    };

    fn member(slot_index: u8) -> ResearchedTeamMember {
        ResearchedTeamMember {
            slot_index,
            id: format!("character-{slot_index}"),
            name: format!("キャラ{slot_index}"),
            element: "炎".into(),
            role: "アタッカー".into(),
            constellation: "無凸".into(),
            image_url: None,
            weapon: format!("武器{slot_index}"),
            weapon_image_url: None,
            artifact: format!("聖遺物{slot_index}"),
            artifact_image_url: None,
            main_stats: "攻撃力 / 元素ダメージ / 会心".into(),
            sub_stats: "会心率 ＞ 会心ダメージ".into(),
            target_stats: vec![
                ResearchedTargetStat {
                    label: "攻撃力".into(),
                    value: "2,000以上".into(),
                    primary: true,
                    note: None,
                },
                ResearchedTargetStat {
                    label: "会心率".into(),
                    value: "70%以上".into(),
                    primary: false,
                    note: None,
                },
            ],
        }
    }

    #[test]
    fn 会話を保存して読み戻せる() {
        let database = Database::open_in_memory().unwrap();
        let mut conversation = database.create_on_demand_conversation().unwrap();
        conversation.messages.push(ResearchMessage {
            role: ResearchMessageRole::User,
            content: "4人を調べたい".into(),
            created_at: timestamp(),
        });
        conversation.updated_at = timestamp();
        database.save_on_demand_conversation(&conversation).unwrap();

        assert_eq!(
            database
                .load_on_demand_conversation(&conversation.session_id)
                .unwrap(),
            Some(conversation)
        );
    }

    #[test]
    fn 完成編成と再利用知識を同時保存できる() {
        let database = Database::open_in_memory().unwrap();
        let mut conversation = database.create_on_demand_conversation().unwrap();
        let now = timestamp();
        let team_id = new_id("team");
        conversation.status = ResearchConversationStatus::Succeeded;
        conversation.team_id = Some(team_id.clone());
        conversation.updated_at = now.clone();
        let record = ResearchedTeamRecord {
            team_id: team_id.clone(),
            session_id: conversation.session_id.clone(),
            title: "保存テスト".into(),
            game_version: "6.0".into(),
            members: (0..4).map(member).collect(),
            sources: vec![ResearchSource {
                title: "根拠".into(),
                url: "https://game8.jp/genshin/12345".into(),
            }],
            warnings: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        };

        database
            .save_researched_team(&conversation, &record)
            .unwrap();

        assert_eq!(
            database.load_researched_team(&team_id).unwrap(),
            Some(record)
        );
        assert_eq!(database.list_researched_team_summaries().unwrap().len(), 1);
        assert_eq!(database.count_on_demand_knowledge().unwrap(), 12);
    }
}
