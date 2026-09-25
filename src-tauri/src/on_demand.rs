use crate::{
    app_server::{
        AppServerSupervisor, ResearchCancellation, collect_on_demand_intake,
        research_on_demand_team,
    },
    database::{Database, new_id, timestamp},
    on_demand_domain::{
        OnDemandResearchProgress, ResearchConversation, ResearchConversationStatus, ResearchIntake,
        ResearchMemberInput, ResearchMessage, ResearchMessageRole, ResearchedTeamDraft,
        ResearchedTeamRecord, ResearchedTeamSummary,
    },
};
use std::collections::HashMap;
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
    database: State<'_, Database>,
    session_id: Option<String>,
    message: String,
) -> Result<ResearchConversation, String> {
    let message = message.trim();
    if message.is_empty() || message.chars().count() > 2_000 {
        return Err("メッセージは1〜2000文字で入力してください".into());
    }
    let mut conversation = match session_id {
        Some(session_id) => database
            .load_on_demand_conversation(&session_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "指定された編成チャットが見つかりません".to_string())?,
        None => database
            .create_on_demand_conversation()
            .map_err(|error| error.to_string())?,
    };
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

    match collect_on_demand_intake(&app, &supervisor, &conversation).await {
        Ok(output) => {
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
    if apply_conditions(&mut conversation, members)? {
        database
            .save_on_demand_conversation(&conversation)
            .map_err(|error| error.to_string())?;
    }
    Ok(conversation)
}

fn apply_conditions(
    conversation: &mut ResearchConversation,
    mut members: Vec<ResearchMemberInput>,
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
        members: members.clone(),
        missing_fields: Vec::new(),
        ready_to_research: true,
    }
    .validate()?;
    if conversation.members == members {
        return Ok(false);
    }

    conversation.missing_fields = members
        .iter()
        .flat_map(|member| {
            let mut missing = Vec::new();
            if member.constellation.is_none() {
                missing.push(format!("{}の凸", member.name));
            }
            if member.weapon.is_none() {
                missing.push(format!("{}の武器", member.name));
            } else if member.refinement.is_none() {
                missing.push(format!("{}の精錬", member.name));
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
                .map(|name| format!("武器は{name}"))
                .unwrap_or_else(|| "武器は指定なし".into());
            let refinement = member
                .refinement
                .map(|value| format!("R{value}"))
                .unwrap_or_else(|| "精錬は指定なし".into());
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
    let mut conversation = database
        .load_on_demand_conversation(&session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "指定された編成チャットが見つかりません".to_string())?;
    let intake = ResearchIntake {
        members: conversation.members.clone(),
        missing_fields: conversation.missing_fields.clone(),
        ready_to_research: conversation.members.len() == 4,
    };
    intake.validate()?;

    let cancellation = ResearchCancellation::default();
    {
        let mut active = coordinator.active.lock().await;
        if active.contains_key(&session_id) {
            return Err("この編成はすでに調査中です".into());
        }
        active.insert(session_id.clone(), cancellation.clone());
    }
    conversation.status = ResearchConversationStatus::Researching;
    conversation.error = None;
    conversation.updated_at = timestamp();
    if let Err(error) = database.save_on_demand_conversation(&conversation) {
        coordinator.active.lock().await.remove(&session_id);
        return Err(error.to_string());
    }
    emit_progress(
        &app,
        &session_id,
        "started",
        "4人の指定を確認し、根拠ページの調査を始めました",
        None,
    );

    let researched = research_on_demand_team(&app, &supervisor, &intake, &cancellation).await;
    let result = match researched {
        Ok(draft) => {
            emit_progress(
                &app,
                &session_id,
                "validating",
                "目標ステータスと根拠ページを確認しています",
                None,
            );
            match finalize_researched_team(&database, &mut conversation, draft) {
                Ok(record) => {
                    emit_progress(
                        &app,
                        &session_id,
                        "completed",
                        "調査結果をこの端末へ保存しました",
                        None,
                    );
                    Ok(record)
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    };

    coordinator.active.lock().await.remove(&session_id);
    if let Err(error) = &result {
        let cancelled = cancellation.is_cancelled();
        conversation.status = if cancelled {
            ResearchConversationStatus::Cancelled
        } else {
            ResearchConversationStatus::Failed
        };
        conversation.error = Some(error.clone());
        conversation.updated_at = timestamp();
        let _ = database.save_on_demand_conversation(&conversation);
        emit_progress(
            &app,
            &session_id,
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

fn finalize_researched_team(
    database: &Database,
    conversation: &mut ResearchConversation,
    draft: ResearchedTeamDraft,
) -> Result<ResearchedTeamRecord, String> {
    draft.validate()?;
    let now = timestamp();
    let existing = if let Some(team_id) = conversation.team_id.as_deref() {
        database
            .load_researched_team(team_id)
            .map_err(|error| error.to_string())?
    } else {
        None
    };
    let record = ResearchedTeamRecord {
        team_id: existing
            .as_ref()
            .map(|record| record.team_id.clone())
            .unwrap_or_else(|| new_id("researched-team")),
        session_id: conversation.session_id.clone(),
        title: draft.title,
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
    conversation.team_id = Some(record.team_id.clone());
    conversation.error = None;
    conversation.updated_at = now;
    conversation.messages.push(ResearchMessage {
        role: ResearchMessageRole::Assistant,
        content: format!(
            "「{}」の調査が完了しました。画像付きカードと目標ステータスを保存しました。",
            record.title
        ),
        created_at: timestamp(),
    });
    database
        .save_researched_team(conversation, &record)
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
) -> Result<Vec<ResearchedTeamSummary>, String> {
    database
        .list_researched_team_summaries()
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
    stage: &str,
    detail: &str,
    member_name: Option<String>,
) {
    let _ = app.emit(
        PROGRESS_EVENT,
        OnDemandResearchProgress {
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

    fn conversation() -> ResearchConversation {
        ResearchConversation {
            session_id: "session-1".into(),
            status: ResearchConversationStatus::Ready,
            messages: Vec::new(),
            members: ["アルレッキーノ", "夜蘭", "ベネット", "鍾離"]
                .into_iter()
                .enumerate()
                .map(|(slot_index, name)| ResearchMemberInput {
                    slot_index: slot_index as u8,
                    name: name.into(),
                    weapon: None,
                    constellation: None,
                    refinement: None,
                })
                .collect(),
            missing_fields: Vec::new(),
            team_id: None,
            error: None,
            created_at: "created".into(),
            updated_at: "updated".into(),
        }
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

        assert!(apply_conditions(&mut conversation, selected).unwrap());
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

        assert!(apply_conditions(&mut conversation, selected).is_err());
        assert!(conversation.messages.is_empty());
    }

    #[test]
    fn 同じ条件なら会話に重複記録しない() {
        let mut conversation = conversation();
        let selected = conversation.members.clone();

        assert!(!apply_conditions(&mut conversation, selected).unwrap());
        assert!(conversation.messages.is_empty());
    }
}
