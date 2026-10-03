use crate::{
    app_server::{
        AppServerSupervisor, ResearchCancellation, collect_on_demand_intake,
        on_demand_research_revision, research_on_demand_team,
    },
    catalog::load_embedded_catalog,
    database::{Database, new_id, timestamp},
    game::GameId,
    on_demand_cache::{CachedTeamResearch, research_cache_key, resolve_team_research},
    on_demand_domain::{
        IntakeAgentOutput, OnDemandResearchProgress, ResearchConversation,
        ResearchConversationStatus, ResearchIntake, ResearchMemberInput, ResearchMessage,
        ResearchMessageRole, ResearchedTeamDraft, ResearchedTeamRecord, ResearchedTeamSummary,
        validated_team_title,
    },
};
use std::{collections::HashMap, future::Future};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

pub const PROGRESS_EVENT: &str = "on-demand-research-progress";

#[derive(Default)]
pub struct OnDemandResearchCoordinator {
    active: Mutex<HashMap<String, ResearchCancellation>>,
}

#[tauri::command]
pub async fn send_on_demand_message(
    app: AppHandle,
    supervisor: State<'_, AppServerSupervisor>,
    coordinator: State<'_, OnDemandResearchCoordinator>,
    database: State<'_, Database>,
    session_id: Option<String>,
    message: String,
    game: Option<GameId>,
) -> Result<ResearchConversation, String> {
    send_message_with_intake(&database, &coordinator, session_id, message, game, |conversation| async move {
        collect_on_demand_intake(&app, &supervisor, &conversation).await
    }).await
}

async fn send_message_with_intake<F, Fut>(
    database: &Database,
    coordinator: &OnDemandResearchCoordinator,
    session_id: Option<String>,
    message: String,
    game: Option<GameId>,
    collect: F,
) -> Result<ResearchConversation, String>
where
    F: FnOnce(ResearchConversation) -> Fut,
    Fut: Future<Output = Result<IntakeAgentOutput, String>>,
{
    let message = message.trim();
    if message.is_empty() || message.chars().count() > 2_000 {
        return Err("メッセージは1〜2000文字で入力してください".into());
    }
    let mut active = coordinator.active.lock().await;
    let mut conversation = match session_id {
        Some(session_id) => database
            .load_on_demand_conversation(&session_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "指定された編成チャットが見つかりません".to_string())?,
        None => database
            .create_on_demand_conversation_for(game.unwrap_or_default())
            .map_err(|error| error.to_string())?,
    };
    if game.is_some_and(|game| game != conversation.game) {
        return Err("会話と要求のゲームが異なります".into());
    }
    if active.contains_key(&conversation.session_id) {
        return Err("受付または調査の完了後に条件を変更してください".into());
    }
    if conversation.status == ResearchConversationStatus::Researching {
        return Err("調査中のため、完了またはキャンセル後に条件を変更してください".into());
    }

    conversation.messages.push(ResearchMessage {
        role: ResearchMessageRole::User,
        content: message.to_owned(),
        created_at: timestamp(),
    });
    conversation.status = ResearchConversationStatus::Collecting;
    conversation.error = None;
    conversation.updated_at = timestamp();
    database
        .save_on_demand_conversation(&conversation)
        .map_err(|error| error.to_string())?;
    active.insert(
        conversation.session_id.clone(),
        ResearchCancellation::default(),
    );
    drop(active);

    let output = collect(conversation.clone()).await;
    let mut active = coordinator.active.lock().await;
    active.remove(&conversation.session_id);
    match output {
        Ok(output) => {
            if output.intake.game != conversation.game {
                return Err("受付結果のゲームが一致しません".into());
            }
            output.intake.validate()?;
            conversation.messages.push(ResearchMessage {
                role: ResearchMessageRole::Assistant,
                content: output.assistant_message,
                created_at: timestamp(),
            });
            conversation.members = output.intake.members;
            conversation.missing_fields = output.intake.missing_fields;
            conversation.status = if output.intake.ready_to_research {
                ResearchConversationStatus::Ready
            } else {
                ResearchConversationStatus::Collecting
            };
            conversation.updated_at = timestamp();
            database
                .save_on_demand_conversation(&conversation)
                .map_err(|error| error.to_string())?;
            Ok(conversation)
        }
        Err(error) => {
            conversation.status = ResearchConversationStatus::Failed;
            conversation.error = Some(error.clone());
            conversation.updated_at = timestamp();
            let _ = database.save_on_demand_conversation(&conversation);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn update_on_demand_conditions(
    coordinator: State<'_, OnDemandResearchCoordinator>,
    database: State<'_, Database>,
    session_id: String,
    members: Vec<ResearchMemberInput>,
    title: Option<String>,
) -> Result<ResearchConversation, String> {
    let active = coordinator.active.lock().await;
    if active.contains_key(&session_id) {
        return Err("調査中のため、完了またはキャンセル後に条件を変更してください".into());
    }
    let mut conversation = database
        .load_on_demand_conversation(&session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "指定された編成チャットが見つかりません".to_string())?;
    if conversation.status == ResearchConversationStatus::Researching {
        return Err("調査中のため、完了またはキャンセル後に条件を変更してください".into());
    }
    if conversation.status == ResearchConversationStatus::Collecting {
        return Err("受付が完了してから条件を変更してください".into());
    }
    if apply_conditions(&mut conversation, members, title)? {
        database
            .save_on_demand_conversation(&conversation)
            .map_err(|error| error.to_string())?;
    }
    Ok(conversation)
}

fn apply_conditions(
    conversation: &mut ResearchConversation,
    mut members: Vec<ResearchMemberInput>,
    title: Option<String>,
) -> Result<bool, String> {
    if conversation.members.len() != 4 || members.len() != 4 {
        return Err("4人のキャラクターが揃ってから条件を選んでください".into());
    }
    for (original, member) in conversation.members.iter().zip(&mut members) {
        if original.slot_index != member.slot_index || original.name.trim() != member.name.trim() {
            return Err("キャラクターの変更はチャットから指定してください".into());
        }
        member.name = original.name.clone();
        member.weapon = member
            .weapon
            .take()
            .map(|weapon| weapon.trim().to_string())
            .filter(|weapon| !weapon.is_empty());
        if member.weapon.is_none() {
            member.refinement = None;
        }
    }
    ResearchIntake {
        game: conversation.game,
        members: members.clone(),
        missing_fields: Vec::new(),
        ready_to_research: true,
    }
    .validate()?;
    let title = title
        .filter(|title| !title.trim().is_empty())
        .map(|title| validated_team_title(&title))
        .transpose()?;
    if conversation.members == members && conversation.title == title {
        return Ok(false);
    }

    let star_rail = conversation.game == GameId::StarRail;
    let weapon_label = if star_rail { "光円錐" } else { "武器" };
    let refinement_label = if star_rail { "重畳" } else { "精錬" };
    conversation.missing_fields = members
        .iter()
        .flat_map(|member| {
            let mut missing = Vec::new();
            if member.constellation.is_none() {
                missing.push(format!("{}の凸", member.name));
            }
            if member.weapon.is_none() {
                missing.push(format!("{}の{weapon_label}", member.name));
            } else if member.refinement.is_none() {
                missing.push(format!("{}の{refinement_label}", member.name));
            }
            missing
        })
        .collect();
    let description = members
        .iter()
        .map(|member| {
            let constellation = member
                .constellation
                .map(|value| format!("{value}凸"))
                .unwrap_or_else(|| "凸は指定なし".into());
            let weapon = member
                .weapon
                .as_deref()
                .map(|name| format!("{weapon_label}は{name}"))
                .unwrap_or_else(|| format!("{weapon_label}は指定なし"));
            let refinement = member
                .refinement
                .map(|value| format!("{}{value}", if star_rail { "S" } else { "R" }))
                .unwrap_or_else(|| format!("{refinement_label}は指定なし"));
            format!(
                "{}：{}、{}、{}",
                member.name, constellation, weapon, refinement
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    conversation.messages.push(ResearchMessage {
        role: ResearchMessageRole::User,
        content: format!("画面で選んだ条件（指定なしは未確定）：\n{description}"),
        created_at: timestamp(),
    });
    conversation.members = members;
    conversation.title = title;
    conversation.status = ResearchConversationStatus::Ready;
    conversation.error = None;
    conversation.updated_at = timestamp();
    Ok(true)
}

#[tauri::command]
pub async fn start_on_demand_research(
    app: AppHandle,
    supervisor: State<'_, AppServerSupervisor>,
    coordinator: State<'_, OnDemandResearchCoordinator>,
    database: State<'_, Database>,
    session_id: String,
) -> Result<ResearchedTeamRecord, String> {
    // Read the input and register the run under the same lock as condition updates.
    let mut active = coordinator.active.lock().await;
    if active.contains_key(&session_id) {
        return Err("この編成はすでに調査中です".into());
    }
    let mut conversation = database
        .load_on_demand_conversation(&session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "指定された編成チャットが見つかりません".to_string())?;
    if conversation.status == ResearchConversationStatus::Collecting {
        return Err("受付が完了してから調査を開始してください".into());
    }
    let intake = ResearchIntake {
        game: conversation.game,
        members: conversation.members.clone(),
        missing_fields: conversation.missing_fields.clone(),
        ready_to_research: conversation.members.len() == 4,
    };
    intake.validate()?;
    let catalog = load_embedded_catalog().map_err(|error| error.to_string())?;
    // スターレイルの名称一覧は現行版の指定ではない。保存された本文の版を維持し、
    // 最長7日の最新結果を使う。保存済み編成からの再調査は必ず本文を取り直す。
    let game_version = (intake.game == GameId::Genshin).then_some(catalog.game_version.as_str());
    let cache_key = research_cache_key(&intake, &catalog, &on_demand_research_revision()?)?;
    // 保存済み編成からの再調査は、同じ条件でもWebで新しい情報を取り直す。
    let refresh = conversation.team_id.is_some();

    let cancellation = ResearchCancellation::default();
    active.insert(session_id.clone(), cancellation.clone());
    conversation.status = ResearchConversationStatus::Researching;
    conversation.error = None;
    conversation.updated_at = timestamp();
    if let Err(error) = database.save_on_demand_conversation(&conversation) {
        active.remove(&session_id);
        return Err(error.to_string());
    }
    drop(active);
    emit_progress(
        &app,
        &session_id,
        conversation.game,
        "started",
        "4人の指定と、この端末で過去に調べた結果を確認しています",
        None,
    );

    let researched = resolve_team_research(
        &database,
        &intake,
        &cache_key,
        game_version,
        refresh,
        |known_sources| {
            let app = &app;
            let session_id = &session_id;
            let supervisor = &supervisor;
            let intake = &intake;
            let cancellation = &cancellation;
            let catalog = &catalog;
            async move {
                emit_progress(
                    app,
                    session_id,
                    intake.game,
                    "researching",
                    if known_sources.is_empty() {
                        "根拠ページを調査しています"
                    } else {
                        "過去に確認した根拠ページを使い、今回の条件で確認し直しています"
                    },
                    None,
                );
                let mut draft =
                    research_on_demand_team(app, supervisor, intake, cancellation, &known_sources)
                        .await?;
                for member in &mut draft.members {
                    member.apply_catalog_images(catalog);
                }
                Ok(draft)
            }
        },
    )
    .await;
    let result = match researched {
        Ok(outcome) if !cancellation.is_cancelled() => {
            emit_progress(
                &app,
                &session_id,
                conversation.game,
                "validating",
                if outcome.reused {
                    if conversation.game == GameId::StarRail {
                        "同じ4人・光円錐・星魂・重畳・固定遺物の調査結果を再利用し、保存しています"
                    } else {
                        "同じ4人・武器・凸・精錬の調査結果を再利用し、保存しています"
                    }
                } else {
                    "目標ステータスと根拠ページを確認しています"
                },
                None,
            );
            match finalize_researched_team(
                &database,
                &mut conversation,
                outcome.draft,
                outcome.cache.as_ref(),
                outcome.reused,
            ) {
                Ok(record) => {
                    emit_progress(
                        &app,
                        &session_id,
                        conversation.game,
                        "completed",
                        "調査結果をこの端末へ保存しました",
                        None,
                    );
                    Ok(record)
                }
                Err(error) => Err(error),
            }
        }
        Ok(_) => Err("調査をキャンセルしました".into()),
        Err(error) => Err(error),
    };

    coordinator.active.lock().await.remove(&session_id);
    if let Err(error) = &result {
        let cancelled = cancellation.is_cancelled();
        let _ = save_research_failure(&database, &mut conversation, error, cancelled);
        emit_progress(
            &app,
            &session_id,
            conversation.game,
            if cancelled { "cancelled" } else { "failed" },
            if cancelled {
                "調査をキャンセルしました"
            } else {
                "調査に失敗しました。条件を確認して再実行できます"
            },
            None,
        );
    }
    result
}

fn save_research_failure(
    database: &Database,
    conversation: &mut ResearchConversation,
    error: &str,
    cancelled: bool,
) -> Result<(), String> {
    conversation.status = if cancelled {
        ResearchConversationStatus::Cancelled
    } else {
        ResearchConversationStatus::Failed
    };
    conversation.error = Some(error.into());
    conversation.updated_at = timestamp();
    database
        .save_on_demand_conversation(conversation)
        .map_err(|error| error.to_string())
}

fn finalize_researched_team(
    database: &Database,
    conversation: &mut ResearchConversation,
    mut draft: ResearchedTeamDraft,
    cache: Option<&CachedTeamResearch>,
    reused: bool,
) -> Result<ResearchedTeamRecord, String> {
    let catalog = load_embedded_catalog().map_err(|error| error.to_string())?;
    for member in &mut draft.members {
        if let Some(build) = &member.star_rail {
            member.artifact = build.tunnel.label();
        }
        member.apply_catalog_images(&catalog);
    }
    if draft.game != conversation.game {
        return Err("調査結果のゲームが一致しません".into());
    }
    draft.normalize_for_intake(&ResearchIntake {
        game: conversation.game,
        members: conversation.members.clone(),
        missing_fields: Vec::new(),
        ready_to_research: true,
    })?;
    if reused {
        draft.warnings.push(
            if conversation.game == GameId::StarRail {
                "過去7日以内に同じ4人・光円錐・星魂・重畳・固定遺物で調べた結果を再利用しました。最新の情報は「条件を変えて再調査」で確認できます。"
            } else {
                "過去7日以内に同じ4人・武器・凸・精錬で調べた結果を再利用しました。最新の情報は「条件を変えて再調査」で確認できます。"
            }.into(),
        );
    }
    let now = timestamp();
    let existing = if let Some(team_id) = conversation.team_id.as_deref() {
        database
            .load_researched_team(team_id)
            .map_err(|error| error.to_string())?
    } else {
        None
    };
    let record = ResearchedTeamRecord {
        game: conversation.game,
        team_reasoning: draft.team_reasoning,
        input_members: Some(conversation.members.clone()),
        team_id: existing
            .as_ref()
            .map(|record| record.team_id.clone())
            .unwrap_or_else(|| new_id("researched-team")),
        session_id: conversation.session_id.clone(),
        title: conversation.title.clone().unwrap_or(draft.title),
        game_version: draft.game_version,
        members: draft.members,
        sources: draft.sources,
        warnings: draft.warnings,
        created_at: existing
            .as_ref()
            .map(|record| record.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now.clone(),
    };
    conversation.status = ResearchConversationStatus::Succeeded;
    conversation.title = Some(record.title.clone());
    conversation.team_id = Some(record.team_id.clone());
    conversation.error = None;
    conversation.updated_at = now;
    conversation.messages.push(ResearchMessage {
        role: ResearchMessageRole::Assistant,
        content: format!(
            "「{}」の{}。画像付きカードと目標ステータスを保存しました。",
            record.title,
            if reused {
                "過去の調査結果を再利用しました"
            } else {
                "調査が完了しました"
            },
        ),
        created_at: timestamp(),
    });
    database
        .save_researched_team_with_cache(conversation, &record, cache)
        .map_err(|error| error.to_string())?;
    Ok(record)
}

#[tauri::command]
pub async fn cancel_on_demand_research(
    coordinator: State<'_, OnDemandResearchCoordinator>,
    session_id: String,
) -> Result<(), String> {
    let active = coordinator.active.lock().await;
    let cancellation = active
        .get(&session_id)
        .ok_or_else(|| "この編成は現在調査中ではありません".to_string())?;
    cancellation.cancel();
    Ok(())
}

#[tauri::command]
pub fn list_researched_teams(
    database: State<'_, Database>,
    game: Option<GameId>,
) -> Result<Vec<ResearchedTeamSummary>, String> {
    database
        .list_researched_team_summaries()
        .map(|teams| {
            teams
                .into_iter()
                .filter(|team| team.game == game.unwrap_or_default())
                .collect()
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn load_researched_team(
    database: State<'_, Database>,
    team_id: String,
) -> Result<Option<ResearchedTeamRecord>, String> {
    database
        .load_researched_team(&team_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn rename_researched_team(
    coordinator: State<'_, OnDemandResearchCoordinator>,
    database: State<'_, Database>,
    team_id: String,
    title: String,
) -> Result<ResearchedTeamRecord, String> {
    let record = database
        .load_researched_team(&team_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "この編成は見つかりません".to_string())?;
    let active = coordinator.active.lock().await;
    if active.contains_key(&record.session_id) {
        return Err("調査中のため、完了またはキャンセル後に編成名を変更してください".into());
    }
    database
        .rename_researched_team(&team_id, &title)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn load_on_demand_conversation(
    database: State<'_, Database>,
    session_id: String,
) -> Result<Option<ResearchConversation>, String> {
    database
        .load_on_demand_conversation(&session_id)
        .map_err(|error| error.to_string())
}

fn emit_progress(
    app: &AppHandle,
    session_id: &str,
    game: GameId,
    stage: &str,
    detail: &str,
    member_name: Option<String>,
) {
    let _ = app.emit(
        PROGRESS_EVENT,
        OnDemandResearchProgress {
            game,
            session_id: session_id.to_owned(),
            stage: stage.to_owned(),
            detail: detail.to_owned(),
            member_name,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn 同じ会話の受付を重ねず失敗後も次の条件を受け付ける() {
        let database = Database::open_in_memory().unwrap();
        let coordinator = OnDemandResearchCoordinator::default();
        let (intake, _) = crate::on_demand_domain::tests::reported_team();
        let conversation = database.create_on_demand_conversation().unwrap();
        let session = conversation.session_id;
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let first = send_message_with_intake(
            &database,
            &coordinator,
            Some(session.clone()),
            "最初の条件".into(),
            None,
            |_| async {
                started_tx.send(()).unwrap();
                release_rx.await.unwrap();
                Err("模擬通信失敗".into())
            },
        );
        let overlap = async {
            started_rx.await.unwrap();
            assert!(coordinator.active.lock().await.contains_key(&session));
            let result = send_message_with_intake(
                &database,
                &coordinator,
                Some(session.clone()),
                "重複した条件".into(),
                None,
                |_| async { panic!("重複受付は外部処理を呼ばない") },
            )
            .await;
            assert!(result.unwrap_err().contains("受付または調査"));
            release_tx.send(()).unwrap();
        };
        let (failed, ()) = tokio::join!(first, overlap);
        assert_eq!(failed.unwrap_err(), "模擬通信失敗");
        assert!(!coordinator.active.lock().await.contains_key(&session));
        let accepted = send_message_with_intake(
            &database,
            &coordinator,
            Some(session.clone()),
            "次の条件".into(),
            None,
            |_| async {
                Ok(IntakeAgentOutput {
                    assistant_message: "条件が揃いました".into(),
                    intake: intake.clone(),
                })
            },
        )
        .await
        .unwrap();
        assert_eq!(accepted.members, intake.members);
        assert_eq!(accepted.status, ResearchConversationStatus::Ready);
        assert!(
            accepted
                .messages
                .iter()
                .all(|m| m.content != "重複した条件")
        );
        assert_eq!(
            database.load_on_demand_conversation(&session).unwrap(),
            Some(accepted)
        );
    }

    #[tokio::test]
    async fn 報告編成の結果を条件と共に保存し再利用と失敗時の保持を確認する() {
        let database = Database::open_in_memory().unwrap();
        let (intake, mut draft) = crate::on_demand_domain::tests::reported_team();
        let mut conversation = database.create_on_demand_conversation().unwrap();
        conversation.members = intake.members.clone();
        conversation.status = ResearchConversationStatus::Ready;
        database.save_on_demand_conversation(&conversation).unwrap();
        draft.members[3].weapon = "蝶の羽化（R１）".into();
        draft.members.reverse();
        let key = research_cache_key(
            &intake,
            &load_embedded_catalog().unwrap(),
            &on_demand_research_revision().unwrap(),
        )
        .unwrap();
        let outcome = resolve_team_research(&database, &intake, &key, None, false, |_| async {
            Ok(draft)
        })
        .await
        .unwrap();
        let record = finalize_researched_team(
            &database,
            &mut conversation,
            outcome.draft.clone(),
            outcome.cache.as_ref(),
            false,
        )
        .unwrap();
        assert_eq!(record.input_members.as_ref(), Some(&intake.members));
        assert_eq!(record.members[3].weapon, "蝶の羽化");
        assert_eq!(record.members[3].refinement, Some(1));
        assert_eq!(
            database.load_researched_team(&record.team_id).unwrap(),
            Some(record.clone())
        );
        let reused = resolve_team_research(&database, &intake, &key, None, false, |_| async {
            panic!("同条件は正規化した保存結果を再利用する")
        })
        .await
        .unwrap();
        assert!(reused.reused);
        let mut invalid = outcome.draft;
        invalid.members[3].weapon = "西風剣".into();
        assert!(
            finalize_researched_team(&database, &mut conversation, invalid, None, false).is_err()
        );
        save_research_failure(&database, &mut conversation, "条件不一致", false).unwrap();
        assert_eq!(
            database.load_researched_team(&record.team_id).unwrap(),
            Some(record)
        );
        let mut changed = intake.clone();
        changed.members[3].constellation = Some(2);
        assert_ne!(
            research_cache_key(
                &changed,
                &load_embedded_catalog().unwrap(),
                &on_demand_research_revision().unwrap()
            )
            .unwrap(),
            key
        );
    }

    #[tokio::test]
    #[ignore = "verify-star-rail-live.ps1から明示した実SQLiteを操作し、別プロセスで再読込する"]
    async fn 実調査の条件と結果を保存して再起動後に読み直せる() {
        use crate::app_server::validate_on_demand_sources;
        use serde_json::{Value, json};
        let read_json = |name: &str| -> Value {
            serde_json::from_slice(&std::fs::read(std::env::var_os(name).unwrap()).unwrap())
                .unwrap()
        };
        let intake: ResearchIntake =
            serde_json::from_value(read_json("GENSHIN_RECO_INTAKE_PATH")).unwrap();
        intake.validate().unwrap();
        assert_eq!(intake.game, GameId::StarRail);
        let database =
            Database::open(std::env::var_os("GENSHIN_RECO_DATABASE_PATH").unwrap()).unwrap();
        let mode = std::env::var("GENSHIN_RECO_STORE_MODE").unwrap();
        let mut conversation = match std::env::var("GENSHIN_RECO_SESSION_ID") {
            Ok(id) => database.load_on_demand_conversation(&id).unwrap().unwrap(),
            Err(_) => {
                assert_eq!(mode, "prepare");
                let mut conversation = database
                    .create_on_demand_conversation_for(intake.game)
                    .unwrap();
                // 受付済みの4人を起点に、画面と同じ条件更新処理を使う。
                conversation.members = intake.members.clone();
                conversation
            }
        };
        assert_eq!(conversation.game, intake.game);
        let previous = conversation
            .team_id
            .as_deref()
            .and_then(|id| database.load_researched_team(id).unwrap());
        let previous_json = serde_json::to_value(&previous).unwrap();
        let catalog = load_embedded_catalog().unwrap();
        let key =
            research_cache_key(&intake, &catalog, &on_demand_research_revision().unwrap()).unwrap();
        match mode.as_str() {
            "prepare" => {
                assert_ne!(conversation.status, ResearchConversationStatus::Researching);
                apply_conditions(
                    &mut conversation,
                    intake.members.clone(),
                    Some("実Web検証：スターレイル".into()),
                )
                .unwrap();
                conversation.status = ResearchConversationStatus::Researching;
                conversation.error = None;
                conversation.updated_at = timestamp();
                database.save_on_demand_conversation(&conversation).unwrap();
            }
            "save" => {
                assert_eq!(conversation.status, ResearchConversationStatus::Researching);
                assert_eq!(conversation.members, intake.members);
                let report = read_json("GENSHIN_RECO_LIVE_REPORT_PATH");
                assert!(
                    matches!(
                        report["verificationKind"].as_str(),
                        Some("operator_verified_fixture" | "app_observation")
                    ),
                    "確認済みの検証用出力とアプリ実出力を区別すること"
                );
                assert_eq!(report["intake"], serde_json::to_value(&intake).unwrap());
                let draft: ResearchedTeamDraft =
                    serde_json::from_value(report["output"].clone()).unwrap();
                let opened: Vec<String> =
                    serde_json::from_value(report["openedUrls"].clone()).unwrap();
                validate_on_demand_sources(intake.game, &draft.sources, &opened).unwrap();
                let outcome =
                    resolve_team_research(&database, &intake, &key, None, true, |_| async {
                        Ok(draft)
                    })
                    .await
                    .unwrap();
                assert!(!outcome.reused);
                let record = finalize_researched_team(
                    &database,
                    &mut conversation,
                    outcome.draft,
                    if report["verificationKind"] == "app_observation" {
                        outcome.cache.as_ref()
                    } else {
                        // 保存境界の確認用出力を、製品の調査結果として再利用しない。
                        None
                    },
                    false,
                )
                .unwrap();
                if let Some(previous) = &previous {
                    assert_eq!(record.team_id, previous.team_id);
                    assert_eq!(record.created_at, previous.created_at);
                }
            }
            "cancel" | "failure" | "abort" => {
                assert_eq!(conversation.status, ResearchConversationStatus::Researching);
                let error = if mode == "abort" {
                    std::env::var("GENSHIN_RECO_FAILURE_MESSAGE").expect("実行失敗の理由があること")
                } else {
                    let report = read_json("GENSHIN_RECO_LIVE_REPORT_PATH");
                    assert!(report.get("output").is_none());
                    assert_eq!(report["cancelled"], mode == "cancel");
                    assert_eq!(report["webSearchEnabled"], false);
                    assert_eq!(report["faultInjected"], true);
                    report["error"]
                        .as_str()
                        .expect("実際の調査エラーがあること")
                        .to_owned()
                };
                save_research_failure(&database, &mut conversation, &error, mode == "cancel")
                    .unwrap();
                let retained = conversation
                    .team_id
                    .as_deref()
                    .and_then(|id| database.load_researched_team(id).unwrap());
                assert_eq!(serde_json::to_value(retained).unwrap(), previous_json);
            }
            "reuse" => {
                let mut fresh = database
                    .create_on_demand_conversation_for(intake.game)
                    .unwrap();
                fresh.members = intake.members.clone();
                let outcome =
                    resolve_team_research(&database, &intake, &key, None, false, |_| async {
                        panic!("同一条件の実保存結果は再検索せず使えること")
                    })
                    .await
                    .unwrap();
                assert!(outcome.reused);
                finalize_researched_team(
                    &database,
                    &mut fresh,
                    outcome.draft,
                    outcome.cache.as_ref(),
                    true,
                )
                .unwrap();
                conversation = fresh;
            }
            "verify" => {
                assert_eq!(conversation.members, intake.members);
                assert_eq!(conversation.status, ResearchConversationStatus::Succeeded);
                let record = previous.as_ref().unwrap();
                assert_eq!(record.input_members.as_ref(), Some(&intake.members));
                ResearchedTeamDraft {
                    game: record.game,
                    team_reasoning: record.team_reasoning.clone(),
                    title: record.title.clone(),
                    game_version: record.game_version.clone(),
                    members: record.members.clone(),
                    sources: record.sources.clone(),
                    warnings: record.warnings.clone(),
                }
                .validate_for_members(&intake.members)
                .unwrap();
            }
            _ => panic!("不明な実保存検証モード"),
        }
        let saved = conversation
            .team_id
            .as_deref()
            .and_then(|id| database.load_researched_team(id).unwrap());
        let report = json!({"mode": mode, "sessionId": conversation.session_id, "conversation": conversation, "result": saved});
        std::fs::write(
            std::env::var_os("GENSHIN_RECO_STORE_REPORT_PATH").unwrap(),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }

    fn conversation() -> ResearchConversation {
        ResearchConversation {
            game: Default::default(),
            session_id: "session-1".into(),
            status: ResearchConversationStatus::Ready,
            messages: Vec::new(),
            members: ["アルレッキーノ", "夜蘭", "ベネット", "鍾離"]
                .into_iter()
                .enumerate()
                .map(|(slot_index, name)| ResearchMemberInput {
                    relics: None,
                    slot_index: slot_index as u8,
                    name: name.into(),
                    weapon: None,
                    constellation: None,
                    refinement: None,
                })
                .collect(),
            title: None,
            missing_fields: Vec::new(),
            team_id: None,
            error: None,
            created_at: "created".into(),
            updated_at: "updated".into(),
        }
    }

    #[test]
    fn スターレイル保存と復元は提案を固定せず失敗とキャンセルで成功結果を保つ() {
        let path = std::env::temp_dir().join(format!("hsr-store-{}.sqlite", new_id("test")));
        let database = Database::open(&path).unwrap();
        let (intake, draft) = crate::star_rail::tests::sample();
        let mut conversation = database
            .create_on_demand_conversation_for(GameId::StarRail)
            .unwrap();
        conversation.members = intake.members;
        conversation.status = ResearchConversationStatus::Ready;
        database.save_on_demand_conversation(&conversation).unwrap();
        let record =
            finalize_researched_team(&database, &mut conversation, draft.clone(), None, false)
                .unwrap();
        assert!(
            record
                .input_members
                .as_ref()
                .unwrap()
                .iter()
                .all(|member| member.relics.is_none())
        );
        let mut other_game = conversation.clone();
        other_game.game = GameId::Genshin;
        assert!(database.save_on_demand_conversation(&other_game).is_err());
        for status in [
            ResearchConversationStatus::Failed,
            ResearchConversationStatus::Cancelled,
        ] {
            conversation.status = status;
            database.save_on_demand_conversation(&conversation).unwrap();
            assert_eq!(
                database.load_researched_team(&record.team_id).unwrap(),
                Some(record.clone())
            );
        }
        let mut invalid = draft;
        invalid.members[0]
            .star_rail
            .as_mut()
            .unwrap()
            .superimposition = 5;
        assert!(
            finalize_researched_team(&database, &mut conversation, invalid, None, false).is_err()
        );
        assert_eq!(
            database.load_researched_team(&record.team_id).unwrap(),
            Some(record.clone())
        );
        drop(database);
        let reopened = Database::open(&path).unwrap();
        let restored = reopened
            .load_on_demand_conversation(&conversation.session_id)
            .unwrap()
            .unwrap();
        assert_eq!(restored.game, GameId::StarRail);
        assert_eq!(restored.members, record.input_members.clone().unwrap());
        assert_eq!(
            reopened.load_researched_team(&record.team_id).unwrap(),
            Some(record)
        );
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn 調査完了時にカタログ画像を返して編成と再利用知識へ保存する() {
        let database = Database::open_in_memory().unwrap();
        let mut conversation = conversation();
        conversation.title = Some("入力した編成名".into());
        database.save_on_demand_conversation(&conversation).unwrap();
        let members = conversation.members.iter().map(|member| serde_json::json!({
            "slotIndex": member.slot_index,
            "id": format!("member-{}", member.slot_index),
            "name": member.name,
            "element": "炎",
            "role": "支援",
            "constellation": "無凸",
            "imageUrl": null,
            "weapon": "西風長槍",
            "weaponImageUrl": "推測された不正なURL",
            "artifact": "旧貴族のしつけ",
            "artifactImageUrl": null,
            "mainStats": "HP / HP / HP",
            "subStats": "HP",
            "targetStats": [
                {"label": "HP", "value": "30,000以上", "primary": true, "note": null},
                {"label": "元素チャージ効率", "value": "180%以上", "primary": false, "note": null}
            ]
        })).collect::<Vec<_>>();
        let draft = serde_json::from_value(serde_json::json!({
            "title": "画像補完テスト", "gameVersion": "7.1", "members": members,
            "sources": [{"title": "根拠", "url": "https://game8.jp/genshin/12345"}],
            "warnings": []
        }))
        .unwrap();
        let record =
            finalize_researched_team(&database, &mut conversation, draft, None, false).unwrap();
        assert_eq!(record.title, "入力した編成名");
        assert!(
            record
                .members
                .iter()
                .all(|member| member.image_url.is_some()
                    && member.weapon_image_url.is_some()
                    && member.artifact_image_url.is_some())
        );
        let connection = database.connection().unwrap();
        let stored_json: String = connection
            .query_row(
                "SELECT result_json FROM researched_teams WHERE team_id = ?1",
                [&record.team_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<ResearchedTeamRecord>(&stored_json).unwrap(),
            record
        );
        let weapon_json: String = connection.query_row(
            "SELECT payload_json FROM on_demand_knowledge WHERE entity_type = 'weapon' AND name = '西風長槍'", [], |row| row.get(0)
        ).unwrap();
        let weapon: serde_json::Value = serde_json::from_str(&weapon_json).unwrap();
        assert_eq!(
            weapon["imageUrl"].as_str(),
            record.members[0].weapon_image_url.as_deref()
        );
    }

    #[test]
    fn uiの凸と武器を保存し指定なしで古い精錬も消す() {
        let mut conversation = conversation();
        conversation.members[1].weapon = Some("若水".into());
        conversation.members[1].refinement = Some(3);
        let mut selected = conversation.members.clone();
        selected[0].constellation = Some(2);
        selected[0].weapon = Some("赤月のシルエット".into());
        selected[1].weapon = None;

        assert!(apply_conditions(&mut conversation, selected, Some(" 蒸発編成 ".into())).unwrap());
        assert_eq!(conversation.title.as_deref(), Some("蒸発編成"));
        assert_eq!(conversation.members[0].constellation, Some(2));
        assert_eq!(
            conversation.members[0].weapon.as_deref(),
            Some("赤月のシルエット")
        );
        assert_eq!(conversation.members[1].weapon, None);
        assert_eq!(conversation.members[1].refinement, None);
        assert_eq!(conversation.status, ResearchConversationStatus::Ready);
        assert!(conversation.missing_fields.contains(&"夜蘭の武器".into()));
        assert!(conversation.messages[0].content.contains("武器は指定なし"));
    }

    #[test]
    fn uiからキャラクター名を変更できない() {
        let mut conversation = conversation();
        let mut selected = conversation.members.clone();
        selected[0].name = "別のキャラ".into();

        assert!(apply_conditions(&mut conversation, selected, None).is_err());
        assert!(conversation.messages.is_empty());
    }

    #[test]
    fn 同じ条件なら会話に重複記録しない() {
        let mut conversation = conversation();
        let selected = conversation.members.clone();

        assert!(!apply_conditions(&mut conversation, selected, None).unwrap());
        assert!(conversation.messages.is_empty());
    }

    #[test]
    fn 編成名だけ変更しても保存対象になる() {
        let mut conversation = conversation();
        let selected = conversation.members.clone();
        assert!(apply_conditions(&mut conversation, selected, Some("新しい名前".into())).unwrap());
        assert_eq!(conversation.title.as_deref(), Some("新しい名前"));
        let selected_again = conversation.members.clone();
        assert!(
            !apply_conditions(&mut conversation, selected_again, Some("新しい名前".into()))
                .unwrap()
        );
    }
}
