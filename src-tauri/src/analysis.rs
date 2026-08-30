use crate::{
    app_server::ResearchCancellation,
    candidate_validation::{
        validate_analysis_input_against_catalog, validate_build_variants_against_catalog,
    },
    catalog::load_embedded_catalog,
    database::{CharacterResearchCacheRecord, Database, SuccessfulAnalysisRecord},
    domain::{
        AnalysisInput, AnalysisStatus, BuildVariant, EvidenceClaim, HostGeneratedIdentity,
        TeamBuildResolution,
    },
    hashing::{
        analysis_input_hash, evidence_snapshot_hash, party_composition_hash, result_hash,
        sha256_canonical,
    },
    reconciler::{VerifiedSourcePage, reconcile_character_research},
    research_provider::{
        CharacterResearchRequest, CodexResearchProvider, ResearchProviderError,
        VerifiedCharacterResearch,
    },
    solver::solve_team_builds,
};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Mutex as StdMutex, time::SystemTime};
use tauri::{Emitter, State};
use tokio::sync::Mutex;

const SOURCE_POLICY_VERSION: &str = "source-policy-v1";
const PROMPT_VERSION: &str = "prompt-v5";
const RESEARCH_SCHEMA_VERSION: &str = "character-research-v2";
const RECONCILER_VERSION: &str = "reconciler-v4";
const SOLVER_VERSION: &str = "solver-v2";
const ANALYSIS_PROGRESS_EVENT: &str = "analysis-progress";

/// 複数の分析実行が同時にWeb調査を始めないための直列化ゲート。
pub struct AnalysisCoordinator {
    gate: Mutex<()>,
    active: StdMutex<Option<ActiveAnalysis>>,
}

#[derive(Clone)]
struct ActiveAnalysis {
    analysis_run_id: String,
    cancellation: ResearchCancellation,
}

impl Default for AnalysisCoordinator {
    fn default() -> Self {
        Self {
            gate: Mutex::new(()),
            active: StdMutex::new(None),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisCommandResult {
    pub result_id: String,
    pub identity: HostGeneratedIdentity,
    pub resolution: TeamBuildResolution,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisProgressEvent {
    analysis_run_id: String,
    status: AnalysisStatus,
    character_id: Option<String>,
    character_stage: Option<String>,
    detail: String,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceSnapshot<'a> {
    analysis_input: &'a AnalysisInput,
    source_pages: Vec<VerifiedSourcePage>,
    evidence_claims: Vec<EvidenceClaim>,
    research_outputs: Vec<Value>,
}

#[tauri::command]
pub async fn start_analysis(
    app: tauri::AppHandle,
    database: State<'_, Database>,
    coordinator: State<'_, AnalysisCoordinator>,
    input: AnalysisInput,
) -> Result<AnalysisCommandResult, String> {
    let _gate = coordinator.gate.lock().await;
    let catalog = load_embedded_catalog().map_err(|error| error.to_string())?;
    validate_analysis_contract(&input, &catalog)?;
    let composition_hash = party_composition_hash(&input).map_err(|error| error.to_string())?;
    let input_hash = analysis_input_hash(&input).map_err(|error| error.to_string())?;
    let run_id = database
        .begin_analysis_run(&input.party_id, &composition_hash, &input_hash)
        .map_err(|error| error.to_string())?;
    let job_ids = match database.create_member_research_jobs(&run_id, &input.members) {
        Ok(job_ids) => job_ids,
        Err(error) => {
            let message = error.to_string();
            let _ = database.update_run_status(&run_id, AnalysisStatus::Failed, Some(&message));
            return Err(message);
        }
    };
    let cancellation = ResearchCancellation::default();
    {
        let mut active = coordinator
            .active
            .lock()
            .map_err(|_| "分析キャンセル状態を取得できませんでした".to_string())?;
        *active = Some(ActiveAnalysis {
            analysis_run_id: run_id.clone(),
            cancellation: cancellation.clone(),
        });
    }

    let result = execute_analysis(
        &app,
        &database,
        &catalog,
        &run_id,
        &input,
        &cancellation,
        &job_ids,
    )
    .await;
    if let Ok(mut active) = coordinator.active.lock()
        && active
            .as_ref()
            .is_some_and(|analysis| analysis.analysis_run_id == run_id)
    {
        *active = None;
    }
    if let Err(error) = &result {
        let status = if cancellation.is_cancelled() {
            AnalysisStatus::Cancelled
        } else {
            AnalysisStatus::Failed
        };
        let _ = database.update_run_status(&run_id, status, Some(error));
        let _ = database.finish_open_research_jobs(&run_id, status, Some(error));
        emit_progress(
            &app,
            &run_id,
            status,
            None,
            None,
            if status == AnalysisStatus::Cancelled {
                "分析をキャンセルしました。"
            } else {
                "分析を完了できませんでした。"
            },
            Some(error.clone()),
        );
    }
    result
}

#[tauri::command]
pub fn cancel_analysis(coordinator: State<'_, AnalysisCoordinator>) -> Result<(), String> {
    let active = coordinator
        .active
        .lock()
        .map_err(|_| "分析キャンセル状態を取得できませんでした".to_string())?;
    let Some(active) = active.as_ref() else {
        return Err("キャンセルできる分析はありません".into());
    };
    active.cancellation.cancel();
    Ok(())
}

async fn execute_analysis(
    app: &tauri::AppHandle,
    database: &Database,
    catalog: &crate::catalog::Catalog,
    run_id: &str,
    input: &AnalysisInput,
    cancellation: &ResearchCancellation,
    job_ids: &[String],
) -> Result<AnalysisCommandResult, String> {
    ensure_not_cancelled(cancellation)?;
    set_status(
        app,
        database,
        run_id,
        AnalysisStatus::StartingCodex,
        "Codex調査環境を開始しています。",
    )?;
    let provider = CodexResearchProvider::new(app.clone());
    let mut candidate_groups = Vec::with_capacity(4);
    let mut source_pages = BTreeMap::<String, VerifiedSourcePage>::new();
    let mut evidence_claims = Vec::new();
    let mut research_outputs = Vec::new();
    let mut research_warnings = Vec::new();

    let (member_0, member_1, member_2, member_3) = tokio::try_join!(
        research_member(
            app,
            database,
            catalog,
            run_id,
            input,
            cancellation,
            &provider,
            0,
            job_ids.first(),
        ),
        research_member(
            app,
            database,
            catalog,
            run_id,
            input,
            cancellation,
            &provider,
            1,
            job_ids.get(1),
        ),
        research_member(
            app,
            database,
            catalog,
            run_id,
            input,
            cancellation,
            &provider,
            2,
            job_ids.get(2),
        ),
        research_member(
            app,
            database,
            catalog,
            run_id,
            input,
            cancellation,
            &provider,
            3,
            job_ids.get(3),
        ),
    )?;

    for (member_index, member_result) in [member_0, member_1, member_2, member_3]
        .into_iter()
        .enumerate()
    {
        let research = member_result.research;
        let variants = member_result.variants;
        for page in research.verified_pages {
            if let Some(existing) = source_pages.insert(page.source_page_id.clone(), page.clone())
                && existing != page
            {
                return Err("同じsourcePageIdへ異なる検証結果が割り当てられました".into());
            }
        }
        for variant in &variants {
            evidence_claims.extend(variant.evidence_claims.clone());
        }
        research_warnings.extend(
            research
                .output
                .warnings
                .iter()
                .map(|warning| format!("{}: {warning}", input.members[member_index].character_id)),
        );
        research_outputs
            .push(serde_json::to_value(&research.output).map_err(|error| error.to_string())?);
        candidate_groups.push(variants);
    }

    ensure_not_cancelled(cancellation)?;
    set_status(
        app,
        database,
        run_id,
        AnalysisStatus::Solving,
        "最大81通りの候補を比較しています。",
    )?;
    let mut resolution = solve_team_builds(candidate_groups).map_err(|error| error.to_string())?;
    resolution.warnings.extend(research_warnings);
    let source_pages = source_pages.into_values().collect::<Vec<_>>();
    let snapshot = EvidenceSnapshot {
        analysis_input: input,
        source_pages,
        evidence_claims,
        research_outputs,
    };
    let snapshot_value = serde_json::to_value(&snapshot).map_err(|error| error.to_string())?;
    let snapshot_hash =
        evidence_snapshot_hash(&snapshot_value).map_err(|error| error.to_string())?;
    let resolution_hash = result_hash(
        &analysis_input_hash(input).map_err(|error| error.to_string())?,
        &snapshot_hash,
        SOLVER_VERSION,
    )
    .map_err(|error| error.to_string())?;
    let now = host_timestamp();
    let identity = HostGeneratedIdentity {
        analysis_run_id: run_id.to_string(),
        party_composition_hash: party_composition_hash(input).map_err(|error| error.to_string())?,
        analysis_input_hash: analysis_input_hash(input).map_err(|error| error.to_string())?,
        evidence_snapshot_hash: snapshot_hash,
        result_hash: resolution_hash,
        checked_at: now.clone(),
        created_at: now,
    };

    ensure_not_cancelled(cancellation)?;
    set_status(
        app,
        database,
        run_id,
        AnalysisStatus::Persisting,
        "検証済み結果を保存しています。",
    )?;
    let result_id = database
        .persist_successful_result(SuccessfulAnalysisRecord {
            identity: identity.clone(),
            evidence_snapshot: snapshot_value,
            result: resolution.clone(),
        })
        .map_err(|error| error.to_string())?;
    emit_progress(
        app,
        run_id,
        AnalysisStatus::Succeeded,
        None,
        None,
        "分析と保存が完了しました。",
        None,
    );
    Ok(AnalysisCommandResult {
        result_id,
        identity,
        resolution,
    })
}

struct MemberResearchResult {
    research: VerifiedCharacterResearch,
    variants: Vec<BuildVariant>,
}

#[allow(clippy::too_many_arguments)]
async fn research_member(
    app: &tauri::AppHandle,
    database: &Database,
    catalog: &crate::catalog::Catalog,
    run_id: &str,
    input: &AnalysisInput,
    cancellation: &ResearchCancellation,
    provider: &CodexResearchProvider,
    member_index: usize,
    job_id: Option<&String>,
) -> Result<MemberResearchResult, String> {
    ensure_not_cancelled(cancellation)?;
    let member = input
        .members
        .get(member_index)
        .ok_or_else(|| "分析対象のキャラクターが4人揃っていません".to_string())?;
    let job_id = job_id.ok_or_else(|| "キャラクター別調査ジョブが不足しています".to_string())?;
    database
        .update_member_research_job(job_id, AnalysisStatus::Researching, 0, None)
        .map_err(|error| error.to_string())?;
    update_character_progress(
        app,
        database,
        run_id,
        AnalysisStatus::Researching,
        &member.character_id,
        "researching",
        "個別本文ページを調査しています。",
    )?;
    let input_hash = analysis_input_hash(input).map_err(|error| error.to_string())?;
    let version_key = sha256_canonical(&input.versions).map_err(|error| error.to_string())?;
    let cached = database
        .load_character_research_cache(
            &member.character_id,
            &input.game_version,
            &version_key,
            &input_hash,
        )
        .map_err(|error| error.to_string())?;
    let prior_research = cached.as_ref().map(|cached| cached.record.output.clone());

    if let Some(cached) = cached.as_ref()
        && (cached.exact_input_match
            || can_reuse_cache_without_web(&cached.record.analysis_input, input))
    {
        let research = VerifiedCharacterResearch {
            output: cached.record.output.clone(),
            verified_pages: cached.record.verified_pages.clone(),
        };
        if let Ok(variants) =
            validate_member_research(&research, &member.character_id, catalog, input)
        {
            let source_count = research.verified_pages.len();
            database
                .update_member_research_job(job_id, AnalysisStatus::Succeeded, source_count, None)
                .map_err(|error| error.to_string())?;
            emit_progress(
                app,
                run_id,
                AnalysisStatus::Reconciling,
                Some(member.character_id.clone()),
                Some("completed".into()),
                "検証済み調査キャッシュを再利用しました。",
                None,
            );
            return Ok(MemberResearchResult { research, variants });
        }
    }

    if prior_research.is_some() {
        update_character_progress(
            app,
            database,
            run_id,
            AnalysisStatus::Researching,
            &member.character_id,
            "researching",
            "保存済み根拠を使って変更点を再調査しています。",
        )?;
    }
    let mut last_validation_error = None;
    for attempt in 0..=1 {
        let research_result = provider
            .research_verified_cancellable(
                CharacterResearchRequest {
                    analysis_input: input.clone(),
                    character_id: member.character_id.clone(),
                    prior_research: prior_research.clone(),
                },
                Some(cancellation.clone()),
            )
            .await;
        let research = match research_result {
            Ok(research) => research,
            Err(error @ ResearchProviderError::EvidenceVerification(_)) if attempt == 0 => {
                last_validation_error = Some(error.to_string());
                database
                    .update_member_research_job(job_id, AnalysisStatus::Researching, 0, None)
                    .map_err(|error| error.to_string())?;
                update_character_progress(
                    app,
                    database,
                    run_id,
                    AnalysisStatus::Researching,
                    &member.character_id,
                    "researching",
                    "根拠情報の整合性を直すため再調査しています。",
                )?;
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        ensure_not_cancelled(cancellation)?;
        let source_count = research.verified_pages.len();
        database
            .update_member_research_job(
                job_id,
                AnalysisStatus::VerifyingSources,
                source_count,
                None,
            )
            .map_err(|error| error.to_string())?;
        update_character_progress(
            app,
            database,
            run_id,
            AnalysisStatus::VerifyingSources,
            &member.character_id,
            "verifying",
            "本文取得イベントと構造化出力を照合しています。",
        )?;
        let validation = validate_member_research(&research, &member.character_id, catalog, input);
        match validation {
            Ok(variants) => {
                database
                    .save_character_research_cache(
                        &input_hash,
                        &version_key,
                        &CharacterResearchCacheRecord {
                            analysis_input: input.clone(),
                            output: research.output.clone(),
                            verified_pages: research.verified_pages.clone(),
                        },
                    )
                    .map_err(|error| error.to_string())?;
                database
                    .update_member_research_job(
                        job_id,
                        AnalysisStatus::Succeeded,
                        source_count,
                        None,
                    )
                    .map_err(|error| error.to_string())?;
                emit_progress(
                    app,
                    run_id,
                    AnalysisStatus::Reconciling,
                    Some(member.character_id.clone()),
                    Some("completed".into()),
                    "候補と根拠の照合が完了しました。",
                    None,
                );
                return Ok(MemberResearchResult { research, variants });
            }
            Err(error) if attempt == 0 => {
                last_validation_error = Some(error);
                database
                    .update_member_research_job(job_id, AnalysisStatus::Researching, 0, None)
                    .map_err(|error| error.to_string())?;
                update_character_progress(
                    app,
                    database,
                    run_id,
                    AnalysisStatus::Researching,
                    &member.character_id,
                    "researching",
                    "候補と根拠の矛盾を解消するため再調査しています。",
                )?;
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_validation_error.unwrap_or_else(|| "候補を検証できませんでした".into()))
}

fn validate_member_research(
    research: &VerifiedCharacterResearch,
    character_id: &str,
    catalog: &crate::catalog::Catalog,
    input: &AnalysisInput,
) -> Result<Vec<BuildVariant>, String> {
    let variants = reconcile_character_research(&research.output, &research.verified_pages, input)
        .map_err(|error| error.to_string())?;
    validate_build_variants_against_catalog(character_id, &variants, catalog)
        .map_err(|error| error.to_string())?;
    Ok(variants)
}

fn can_reuse_cache_without_web(cached: &AnalysisInput, current: &AnalysisInput) -> bool {
    cached.game_version == current.game_version
        && cached.assumptions == current.assumptions
        && cached.versions == current.versions
        && cached.members.iter().zip(current.members.iter()).all(
            |(cached_member, current_member)| {
                cached_member.slot_index == current_member.slot_index
                    && cached_member.character_id == current_member.character_id
                    && cached_member.weapon_id == current_member.weapon_id
            },
        )
}

fn validate_analysis_contract(
    input: &AnalysisInput,
    catalog: &crate::catalog::Catalog,
) -> Result<(), String> {
    validate_analysis_input_against_catalog(input, catalog).map_err(|error| error.to_string())?;
    let versions = &input.versions;
    for (label, actual, expected) in [
        (
            "catalogVersion",
            versions.catalog_version.as_str(),
            catalog.schema_version.as_str(),
        ),
        (
            "sourcePolicyVersion",
            versions.source_policy_version.as_str(),
            SOURCE_POLICY_VERSION,
        ),
        (
            "promptVersion",
            versions.prompt_version.as_str(),
            PROMPT_VERSION,
        ),
        (
            "schemaVersion",
            versions.schema_version.as_str(),
            RESEARCH_SCHEMA_VERSION,
        ),
        (
            "reconcilerVersion",
            versions.reconciler_version.as_str(),
            RECONCILER_VERSION,
        ),
        (
            "solverVersion",
            versions.solver_version.as_str(),
            SOLVER_VERSION,
        ),
    ] {
        if actual != expected {
            return Err(format!(
                "{label}が実装版と一致しません（{actual} != {expected}）"
            ));
        }
    }
    Ok(())
}

fn set_status(
    app: &tauri::AppHandle,
    database: &Database,
    run_id: &str,
    status: AnalysisStatus,
    detail: &str,
) -> Result<(), String> {
    database
        .update_run_status(run_id, status, None)
        .map_err(|error| error.to_string())?;
    emit_progress(app, run_id, status, None, None, detail, None);
    Ok(())
}

fn update_character_progress(
    app: &tauri::AppHandle,
    database: &Database,
    run_id: &str,
    status: AnalysisStatus,
    character_id: &str,
    character_stage: &str,
    detail: &str,
) -> Result<(), String> {
    database
        .update_run_status(run_id, status, None)
        .map_err(|error| error.to_string())?;
    emit_progress(
        app,
        run_id,
        status,
        Some(character_id.to_string()),
        Some(character_stage.to_string()),
        detail,
        None,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_progress(
    app: &tauri::AppHandle,
    run_id: &str,
    status: AnalysisStatus,
    character_id: Option<String>,
    character_stage: Option<String>,
    detail: &str,
    error: Option<String>,
) {
    let _ = app.emit(
        ANALYSIS_PROGRESS_EVENT,
        AnalysisProgressEvent {
            analysis_run_id: run_id.to_string(),
            status,
            character_id,
            character_stage,
            detail: detail.to_string(),
            error,
        },
    );
}

fn host_timestamp() -> String {
    let elapsed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", elapsed.as_secs(), elapsed.subsec_nanos())
}

fn ensure_not_cancelled(cancellation: &ResearchCancellation) -> Result<(), String> {
    if cancellation.is_cancelled() {
        Err("分析がキャンセルされました".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AnalysisVersions, FixedAssumptions, PartyMemberInput};

    fn valid_input() -> AnalysisInput {
        let catalog = load_embedded_catalog().unwrap();
        let characters = catalog
            .characters
            .iter()
            .filter(|character| !character.id.starts_with("traveler-"))
            .take(4)
            .collect::<Vec<_>>();
        let members = std::array::from_fn(|slot_index| {
            let character = characters[slot_index];
            let weapon = catalog
                .weapons
                .iter()
                .find(|weapon| weapon.weapon_type == character.weapon_type)
                .unwrap();
            PartyMemberInput {
                slot_index: slot_index as u8,
                character_id: character.id.clone(),
                weapon_id: weapon.id.clone(),
                refinement: 1,
                constellation: 0,
            }
        });
        AnalysisInput {
            party_id: "party-analysis-test".into(),
            party_name: "分析テスト".into(),
            game_version: catalog.game_version,
            members,
            assumptions: FixedAssumptions {
                character_level: 90,
                weapon_level: 90,
                artifact_level: 20,
                artifact_rarity: 5,
                sheet_timing: "pre_combat".into(),
                final_ascension: true,
                all_talents_available: true,
                witch_teaching_when_applicable: true,
            },
            versions: AnalysisVersions {
                catalog_version: "catalog-v2".into(),
                source_policy_version: SOURCE_POLICY_VERSION.into(),
                prompt_version: PROMPT_VERSION.into(),
                schema_version: RESEARCH_SCHEMA_VERSION.into(),
                reconciler_version: RECONCILER_VERSION.into(),
                solver_version: SOLVER_VERSION.into(),
            },
        }
    }

    #[test]
    fn カタログと実装版に一致する分析入力を受理する() {
        let catalog = load_embedded_catalog().unwrap();
        validate_analysis_contract(&valid_input(), &catalog).unwrap();
    }

    #[test]
    fn 古いsolver版の分析入力を開始前に拒否する() {
        let catalog = load_embedded_catalog().unwrap();
        let mut input = valid_input();
        input.versions.solver_version = "solver-v0".into();

        let error = validate_analysis_contract(&input, &catalog).unwrap_err();

        assert!(error.contains("solverVersion"));
    }

    #[test]
    fn 凸と精錬だけの変更は検証済み調査を再利用できる() {
        let cached = valid_input();
        let mut changed = cached.clone();
        changed.members[0].constellation = 2;
        changed.members[1].refinement = 5;
        assert!(can_reuse_cache_without_web(&cached, &changed));

        changed.members[2].character_id = "different-character".into();
        assert!(!can_reuse_cache_without_web(&cached, &changed));
    }
}
