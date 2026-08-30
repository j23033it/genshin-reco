use crate::database::{Database, PartyDraft, PartySummary};
use crate::domain::TeamBuildResolution;
use tauri::{AppHandle, Manager, State};

/// DBエラーをRendererへ返すための、日本語の安全な文字列へ変換する。
fn database_error_message(error: crate::database::DatabaseError) -> String {
    error.to_string()
}

#[tauri::command]
pub fn save_party_draft(database: State<'_, Database>, draft: PartyDraft) -> Result<(), String> {
    save_party_draft_impl(&database, draft)
}

#[tauri::command]
pub fn load_party_draft(
    database: State<'_, Database>,
    party_id: String,
) -> Result<Option<PartyDraft>, String> {
    load_party_draft_impl(&database, &party_id)
}

#[tauri::command]
pub fn list_party_drafts(database: State<'_, Database>) -> Result<Vec<PartySummary>, String> {
    list_party_drafts_impl(&database)
}

#[tauri::command]
pub fn delete_party_draft(database: State<'_, Database>, party_id: String) -> Result<(), String> {
    delete_party_draft_impl(&database, &party_id)
}

#[tauri::command]
pub fn load_current_analysis_result(
    database: State<'_, Database>,
    party_id: String,
) -> Result<Option<TeamBuildResolution>, String> {
    database
        .load_current_result(&party_id)
        .map_err(database_error_message)
}

#[tauri::command]
pub fn save_analysis_variant_selection(
    database: State<'_, Database>,
    party_id: String,
    character_id: String,
    variant_id: String,
) -> Result<TeamBuildResolution, String> {
    database
        .save_user_selection(&party_id, &character_id, &variant_id)
        .map_err(database_error_message)
}

fn save_party_draft_impl(database: &Database, draft: PartyDraft) -> Result<(), String> {
    database.save_party(draft).map_err(database_error_message)
}

fn load_party_draft_impl(
    database: &Database,
    party_id: &str,
) -> Result<Option<PartyDraft>, String> {
    database
        .load_party(party_id)
        .map_err(database_error_message)
}

fn list_party_drafts_impl(database: &Database) -> Result<Vec<PartySummary>, String> {
    database.list_parties().map_err(database_error_message)
}

fn delete_party_draft_impl(database: &Database, party_id: &str) -> Result<(), String> {
    database
        .delete_party(party_id)
        .map_err(database_error_message)
}

/// アプリデータディレクトリ内のDBを開き、Tauriのmanaged stateへ登録する。
pub fn initialize_database(app: &AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("アプリデータディレクトリを取得できませんでした: {error}"))?;
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|error| format!("アプリデータディレクトリを作成できませんでした: {error}"))?;
    let database_path = app_data_dir.join("genshin-reco.sqlite3");
    let database = Database::open(&database_path)
        .map_err(|error| format!("データベースを初期化できませんでした: {error}"))?;
    app.manage(database);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::PartyMemberDraft;

    fn draft() -> PartyDraft {
        PartyDraft::new(
            "party-command-test",
            "コマンド検証",
            (0..4)
                .map(|slot_index| PartyMemberDraft {
                    slot_index,
                    character_id: (slot_index < 3).then(|| format!("char-{slot_index}")),
                    weapon_id: (slot_index < 3).then(|| format!("weapon-{slot_index}")),
                    refinement: 1,
                    constellation: 0,
                    intent: None,
                })
                .collect(),
        )
    }

    #[test]
    fn 編成下書きcommandで保存読込一覧を扱える() {
        let database = Database::open_in_memory().unwrap();
        let expected = draft();

        save_party_draft_impl(&database, expected.clone()).unwrap();

        assert_eq!(
            load_party_draft_impl(&database, &expected.party_id).unwrap(),
            Some(expected)
        );
        let summaries = list_party_drafts_impl(&database).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].party_id, "party-command-test");
    }

    #[test]
    fn dbエラーは日本語文字列へ変換される() {
        let database = Database::open_in_memory().unwrap();

        let error = load_party_draft_impl(&database, " ").unwrap_err();

        assert!(error.contains("編成IDは必須です"));
    }

    #[test]
    fn 現在結果がない編成はnull相当を返す() {
        let database = Database::open_in_memory().unwrap();
        database.save_party(draft()).unwrap();

        assert_eq!(
            database.load_current_result("party-command-test").unwrap(),
            None
        );
    }

    #[test]
    fn 編成削除commandで一覧と読込から除外する() {
        let database = Database::open_in_memory().unwrap();
        let expected = draft();
        save_party_draft_impl(&database, expected.clone()).unwrap();

        delete_party_draft_impl(&database, &expected.party_id).unwrap();

        assert!(list_party_drafts_impl(&database).unwrap().is_empty());
        assert_eq!(
            load_party_draft_impl(&database, &expected.party_id).unwrap(),
            None
        );
    }
}
