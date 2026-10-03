use crate::game::GameId;
use crate::{
    catalog::load_embedded_catalog,
    database::{Database, DatabaseError, new_id, timestamp},
    on_demand_cache::{CachedTeamResearch, save_research_cache},
    on_demand_domain::{
        ResearchConversation, ResearchConversationStatus, ResearchedTeamRecord,
        ResearchedTeamSummary, validated_team_title,
    },
};
use rusqlite::{OptionalExtension, params};

pub(crate) fn knowledge_type(game: GameId, entity_type: &str) -> String {
    match game {
        GameId::Genshin => entity_type.to_owned(),
        GameId::StarRail => format!("star_rail:{entity_type}"),
    }
}

fn status_text(status: ResearchConversationStatus) -> Result<String, DatabaseError> {
    serde_json::to_value(status)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| DatabaseError::Invalid("会話状態を文字列化できません".into()))
}

impl Database {
    pub fn create_on_demand_conversation(&self) -> Result<ResearchConversation, DatabaseError> {
        self.create_on_demand_conversation_for(GameId::Genshin)
    }

    pub fn create_on_demand_conversation_for(
        &self,
        game: GameId,
    ) -> Result<ResearchConversation, DatabaseError> {
        let now = timestamp();
        let conversation = ResearchConversation {
            game,
            session_id: new_id("research-session"),
            status: ResearchConversationStatus::Collecting,
            messages: Vec::new(),
            members: Vec::new(),
            title: None,
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
        let existing: Option<String> = connection
            .query_row(
                "SELECT conversation_json FROM on_demand_research_sessions WHERE session_id = ?1",
                [&conversation.session_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(json) = existing {
            let previous: ResearchConversation = serde_json::from_str(&json)?;
            if previous.game != conversation.game {
                return Err(DatabaseError::Invalid(
                    "会話のゲームは変更できません".into(),
                ));
            }
        }
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
        json.map(|json| {
            let mut conversation: ResearchConversation = serde_json::from_str(&json)?;
            restore_conversation_cone_names(&mut conversation);
            Ok(conversation)
        })
        .transpose()
    }

    pub fn save_researched_team(
        &self,
        conversation: &ResearchConversation,
        record: &ResearchedTeamRecord,
    ) -> Result<(), DatabaseError> {
        self.save_researched_team_with_cache(conversation, record, None)
    }

    pub(crate) fn save_researched_team_with_cache(
        &self,
        conversation: &ResearchConversation,
        record: &ResearchedTeamRecord,
        cache: Option<&CachedTeamResearch>,
    ) -> Result<(), DatabaseError> {
        validate_conversation(conversation)?;
        if conversation.game != record.game
            || conversation.session_id != record.session_id
            || conversation.team_id.as_deref() != Some(record.team_id.as_str())
            || conversation.status != ResearchConversationStatus::Succeeded
        {
            return Err(DatabaseError::Invalid(
                "保存する会話と編成結果の識別子または状態が一致しません".into(),
            ));
        }
        let draft = crate::on_demand_domain::ResearchedTeamDraft {
            game: record.game,
            team_reasoning: record.team_reasoning.clone(),
            title: record.title.clone(),
            game_version: record.game_version.clone(),
            members: record.members.clone(),
            sources: record.sources.clone(),
            warnings: record.warnings.clone(),
        };
        // Old Genshin records without an input snapshot remain readable/saveable.
        // New results for both games must preserve the request at persistence too.
        if record.game == GameId::StarRail || record.input_members.is_some() {
            if record.input_members.as_ref() != Some(&conversation.members) {
                return Err(DatabaseError::Invalid(
                    "保存条件が会話と一致しません".into(),
                ));
            }
            draft
                .validate_for_members(&conversation.members)
                .map_err(DatabaseError::Invalid)?;
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
        // 再利用や編成名の変更では、根拠を調べた日時を新しくしない。
        let researched_at = cache.map_or(record.updated_at.as_str(), |cache| {
            cache.researched_at.as_str()
        });
        for member in &record.members {
            upsert_knowledge(
                &transaction,
                &knowledge_type(record.game, "character"),
                &member.name,
                &record.game_version,
                &serde_json::to_string(member)?,
                &sources_json,
                researched_at,
            )?;
            upsert_knowledge(
                &transaction,
                &knowledge_type(record.game, "weapon"),
                &member.weapon,
                &record.game_version,
                &serde_json::to_string(&serde_json::json!({
                    "name": member.weapon,
                    "imageUrl": member.weapon_image_url,
                }))?,
                &sources_json,
                researched_at,
            )?;
            upsert_knowledge(
                &transaction,
                &knowledge_type(record.game, "artifact"),
                &member.artifact,
                &record.game_version,
                &serde_json::to_string(&serde_json::json!({
                    "name": member.artifact,
                    "imageUrl": member.artifact_image_url,
                }))?,
                &sources_json,
                researched_at,
            )?;
        }
        if let Some(cache) = cache {
            save_research_cache(&transaction, cache)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_researched_team_summaries(
        &self,
    ) -> Result<Vec<ResearchedTeamSummary>, DatabaseError> {
        let catalog =
            load_embedded_catalog().map_err(|error| DatabaseError::Invalid(error.to_string()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT result_json FROM researched_teams ORDER BY updated_at DESC, team_id DESC",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut summaries = Vec::new();
        for row in rows {
            let mut record: ResearchedTeamRecord = serde_json::from_str(&row?)?;
            for member in &mut record.members {
                member.apply_catalog_images(&catalog);
            }
            summaries.push(ResearchedTeamSummary {
                game: record.game,
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
        json.map(|json| {
            let mut record: ResearchedTeamRecord = serde_json::from_str(&json)?;
            restore_record_cone_names(&mut record);
            let catalog = load_embedded_catalog()
                .map_err(|error| DatabaseError::Invalid(error.to_string()))?;
            for member in &mut record.members {
                member.apply_catalog_images(&catalog);
            }
            Ok(record)
        })
        .transpose()
    }

    pub fn rename_researched_team(
        &self,
        team_id: &str,
        title: &str,
    ) -> Result<ResearchedTeamRecord, DatabaseError> {
        let title = validated_team_title(title).map_err(DatabaseError::Invalid)?;
        let catalog =
            load_embedded_catalog().map_err(|error| DatabaseError::Invalid(error.to_string()))?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let record_json: String = transaction
            .query_row(
                "SELECT result_json FROM researched_teams WHERE team_id = ?1",
                params![team_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| DatabaseError::Invalid("この編成は見つかりません".into()))?;
        let mut record: ResearchedTeamRecord = serde_json::from_str(&record_json)?;
        restore_record_cone_names(&mut record);
        let conversation_json: String = transaction
            .query_row(
                "SELECT conversation_json FROM on_demand_research_sessions WHERE session_id = ?1",
                params![record.session_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| DatabaseError::Invalid("編成の会話が見つかりません".into()))?;
        let mut conversation: ResearchConversation = serde_json::from_str(&conversation_json)?;
        restore_conversation_cone_names(&mut conversation);
        let now = timestamp();
        record.title = title.clone();
        record.updated_at = now.clone();
        conversation.title = Some(title);
        conversation.updated_at = now;
        transaction.execute(
            "UPDATE researched_teams SET title = ?2, result_json = ?3, updated_at = ?4 WHERE team_id = ?1",
            params![record.team_id, record.title, serde_json::to_string(&record)?, record.updated_at],
        )?;
        transaction.execute(
            "UPDATE on_demand_research_sessions SET conversation_json = ?2, updated_at = ?3 WHERE session_id = ?1",
            params![conversation.session_id, serde_json::to_string(&conversation)?, conversation.updated_at],
        )?;
        transaction.commit()?;
        for member in &mut record.members {
            member.apply_catalog_images(&catalog);
        }
        Ok(record)
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

fn restore_cone_name(name: &mut String) {
    // 旧カタログの同一IDに対する誤表記だけを補正する。未知の装備は変更しない。
    let corrected = match name.as_str() {
        "逃げ場のない夢" => "逃げ場なし",
        "天が落ちる" => "天傾",
        "孤独の癒し" => "孤独の癒やし",
        _ => return,
    };
    *name = corrected.into();
}

fn restore_conversation_cone_names(conversation: &mut ResearchConversation) {
    if conversation.game == GameId::StarRail {
        for member in &mut conversation.members {
            if let Some(name) = &mut member.weapon {
                restore_cone_name(name);
            }
        }
    }
}

fn restore_record_cone_names(record: &mut ResearchedTeamRecord) {
    if record.game != GameId::StarRail {
        return;
    }
    if let Some(inputs) = &mut record.input_members {
        for input in inputs {
            if let Some(name) = &mut input.weapon {
                restore_cone_name(name);
            }
        }
    }
    for member in &mut record.members {
        restore_cone_name(&mut member.weapon);
        if let Some(build) = &mut member.star_rail {
            restore_cone_name(&mut build.light_cone);
        }
    }
}

fn validate_conversation(conversation: &ResearchConversation) -> Result<(), DatabaseError> {
    if conversation.session_id.trim().is_empty() {
        return Err(DatabaseError::Invalid("会話IDは必須です".into()));
    }
    if let Some(title) = conversation.title.as_deref() {
        validated_team_title(title).map_err(DatabaseError::Invalid)?;
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
            updated_at = excluded.updated_at
         WHERE CAST(excluded.updated_at AS REAL) >= CAST(on_demand_knowledge.updated_at AS REAL)",
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

    #[test]
    fn 旧光円錐名の保存結果と会話を正式名称で再調査できる() {
        use crate::on_demand_domain::ResearchIntake;
        for (old, current, character) in [
            ("逃げ場のない夢", "逃げ場なし", "ホタル"),
            ("天が落ちる", "天傾", "ホタル"),
            ("孤独の癒し", "孤独の癒やし", "ルカ"),
        ] {
            let database = Database::open_in_memory().unwrap();
            let (mut intake, mut draft) = crate::star_rail::tests::sample();
            intake.members[0].name = character.into();
            intake.members[0].weapon = Some(current.into());
            draft.members[0].name = character.into();
            draft.members[0].weapon = current.into();
            draft.members[0].star_rail.as_mut().unwrap().light_cone = current.into();
            intake.validate().unwrap();
            draft.validate_for_members(&intake.members).unwrap();
            let mut conversation = database
                .create_on_demand_conversation_for(GameId::StarRail)
                .unwrap();
            conversation.members = intake.members.clone();
            conversation.status = ResearchConversationStatus::Succeeded;
            conversation.team_id = Some("legacy-team".into());
            let mut value = serde_json::to_value(&draft).unwrap();
            value["teamId"] = serde_json::json!("legacy-team");
            value["sessionId"] = serde_json::json!(conversation.session_id);
            value["inputMembers"] = serde_json::json!(intake.members);
            value["createdAt"] = serde_json::json!(conversation.created_at);
            value["updatedAt"] = serde_json::json!(conversation.updated_at);
            let record: ResearchedTeamRecord = serde_json::from_value(value).unwrap();
            database
                .save_researched_team(&conversation, &record)
                .unwrap();
            let expected_record = database
                .load_researched_team(&record.team_id)
                .unwrap()
                .unwrap();
            // 更新前のSQLiteに実際に保存されていた表記を再現する。
            let mut legacy_record = record.clone();
            legacy_record.input_members.as_mut().unwrap()[0].weapon = Some(old.into());
            legacy_record.members[0].weapon = old.into();
            legacy_record.members[0]
                .star_rail
                .as_mut()
                .unwrap()
                .light_cone = old.into();
            let mut legacy_conversation = conversation.clone();
            legacy_conversation.members[0].weapon = Some(old.into());
            let connection = database.connection().unwrap();
            connection
                .execute(
                    "UPDATE researched_teams SET result_json=?1 WHERE team_id=?2",
                    params![
                        serde_json::to_string(&legacy_record).unwrap(),
                        record.team_id
                    ],
                )
                .unwrap();
            connection.execute("UPDATE on_demand_research_sessions SET conversation_json=?1 WHERE session_id=?2", params![serde_json::to_string(&legacy_conversation).unwrap(), conversation.session_id]).unwrap();
            drop(connection);
            let restored = database
                .load_on_demand_conversation(&conversation.session_id)
                .unwrap()
                .unwrap();
            assert_eq!(restored, conversation);
            ResearchIntake {
                game: restored.game,
                members: restored.members,
                missing_fields: vec![],
                ready_to_research: true,
            }
            .validate()
            .unwrap();
            assert_eq!(
                database
                    .load_researched_team(&record.team_id)
                    .unwrap()
                    .unwrap(),
                expected_record
            );
            let renamed = database
                .rename_researched_team(&record.team_id, "名称変更")
                .unwrap();
            assert_eq!(renamed.members[0].weapon, current);
            assert_eq!(
                renamed.input_members.unwrap()[0].weapon.as_deref(),
                Some(current)
            );
            let connection = database.connection().unwrap();
            let stored: String = connection
                .query_row(
                    "SELECT result_json FROM researched_teams WHERE team_id=?1",
                    [&record.team_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!stored.contains(old));
        }
    }

    #[test]
    fn 旧光円錐の補正は原神と未知の名前を変更しない() {
        let database = Database::open_in_memory().unwrap();
        let mut conversation = database.create_on_demand_conversation().unwrap();
        conversation.members = crate::star_rail::tests::sample().0.members;
        conversation.members[0].weapon = Some("天が落ちる".into());
        database.save_on_demand_conversation(&conversation).unwrap();
        assert_eq!(
            database
                .load_on_demand_conversation(&conversation.session_id)
                .unwrap()
                .unwrap(),
            conversation
        );
        conversation.game = GameId::StarRail;
        conversation.members[0].weapon = Some("未登録の固定光円錐".into());
        restore_conversation_cone_names(&mut conversation);
        assert_eq!(
            conversation.members[0].weapon.as_deref(),
            Some("未登録の固定光円錐")
        );
    }
    use crate::on_demand_domain::{
        ResearchMessage, ResearchMessageRole, ResearchSource, ResearchedTargetStat,
        ResearchedTeamMember,
    };

    fn member(slot_index: u8) -> ResearchedTeamMember {
        ResearchedTeamMember {
            refinement: Some(1),
            star_rail: None,
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
    fn 画像は出力済みurlや不一致のidよりカタログの名称を優先する() {
        let catalog = load_embedded_catalog().unwrap();
        let mut member = member(0);
        member.id = "yelan".into();
        member.name = " ヴェスナ ".into();
        member.weapon = "蝶の羽化（R１）".into();
        member.artifact = "紅血の証　４セット".into();
        member.image_url = Some("https://example.com/wrong.png".into());
        member.weapon_image_url = Some("https://example.com/wrong.png".into());
        member.apply_catalog_images(&catalog);

        assert_eq!(
            member.image_url.as_deref(),
            Some("https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Vesna.png")
        );
        assert_eq!(
            member.weapon_image_url.as_deref(),
            Some("https://gi.yatta.moe/assets/UI/UI_EquipIcon_Sword_Samosvist.png")
        );
        assert_eq!(
            member.artifact_image_url.as_deref(),
            Some("https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15047_4.png")
        );
    }

    #[test]
    fn 未登録名や複数装備候補の画像を推測しない() {
        let catalog = load_embedded_catalog().unwrap();
        let mut member = member(0);
        member.id = "yelan".into();
        member.weapon = "若水または西風猟弓".into();
        member.artifact = "絶縁の旗印2セット＋旧貴族のしつけ2セット".into();
        member.image_url = Some("https://example.com/old.png".into());
        member.weapon_image_url = member.image_url.clone();
        member.artifact_image_url = member.image_url.clone();
        member.apply_catalog_images(&catalog);

        assert!(member.image_url.is_none());
        assert!(member.weapon_image_url.is_none());
        assert!(member.artifact_image_url.is_none());

        member.name = "旅人".into();
        member.element = "風元素".into();
        member.apply_catalog_images(&catalog);
        assert!(member.image_url.is_some());
        member.element = "不明".into();
        member.apply_catalog_images(&catalog);
        assert!(member.image_url.is_none());
        member.name = "旅人(氷)".into();
        member.apply_catalog_images(&catalog);
        assert!(member.image_url.is_some());
    }

    #[test]
    fn 画像なしの旧保存結果は再調査せず詳細と一覧で補完する() {
        let database = Database::open_in_memory().unwrap();
        let mut conversation = database.create_on_demand_conversation().unwrap();
        conversation.status = ResearchConversationStatus::Succeeded;
        conversation.team_id = Some("legacy-team".into());
        let mut legacy_member = member(0);
        legacy_member.name = "ヴォジャニーツァ".into();
        legacy_member.weapon = "旋流の讃美歌".into();
        legacy_member.artifact = "絶縁の旗印（4セット）".into();
        let record = ResearchedTeamRecord {
            game: Default::default(),
            team_reasoning: None,
            input_members: None,
            team_id: "legacy-team".into(),
            session_id: conversation.session_id.clone(),
            title: "保存済み編成".into(),
            game_version: "7.1".into(),
            members: vec![legacy_member, member(1), member(2), member(3)],
            sources: Vec::new(),
            warnings: vec!["既存の注意事項".into()],
            created_at: "created".into(),
            updated_at: "updated".into(),
        };
        database
            .save_researched_team(&conversation, &record)
            .unwrap();

        let loaded = database
            .load_researched_team("legacy-team")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.members[0].image_url.as_deref(),
            Some("https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Vodyanitsa.png")
        );
        assert!(loaded.members[0].weapon_image_url.is_some());
        assert!(loaded.members[0].artifact_image_url.is_some());
        assert!(loaded.members[1].image_url.is_none());
        assert_eq!(loaded.warnings, record.warnings);
        assert_eq!(loaded.updated_at, record.updated_at);
        let summaries = database.list_researched_team_summaries().unwrap();
        assert_eq!(
            summaries[0].member_image_urls[0],
            loaded.members[0].image_url
        );
        assert_eq!(summaries[0].member_image_urls.len(), 4);

        // 読み込み時の補完で、保存済みの本文や更新日時を書き換えない。
        let stored_json: String = database
            .connection()
            .unwrap()
            .query_row(
                "SELECT result_json FROM researched_teams WHERE team_id = 'legacy-team'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<ResearchedTeamRecord>(&stored_json).unwrap(),
            record
        );
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
            game: Default::default(),
            team_reasoning: None,
            input_members: None,
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

        let renamed = database
            .rename_researched_team(&team_id, " 変更後 ")
            .unwrap();
        assert_eq!(renamed.title, "変更後");
        assert_eq!(
            database
                .load_researched_team(&team_id)
                .unwrap()
                .unwrap()
                .title,
            "変更後"
        );
        assert_eq!(
            database.list_researched_team_summaries().unwrap()[0].title,
            "変更後"
        );
        assert_eq!(
            database
                .load_on_demand_conversation(&conversation.session_id)
                .unwrap()
                .unwrap()
                .title
                .as_deref(),
            Some("変更後")
        );
        assert!(database.rename_researched_team(&team_id, " ").is_err());
    }
}
