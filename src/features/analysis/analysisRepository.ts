import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnalysisInput,
  AnalysisStatus,
  HostGeneratedIdentity,
  TeamBuildResolution,
} from "../../domain/analysisTypes";
import type { Catalog } from "../../domain/catalogTypes";
import type { PartyDraft } from "../party";

export interface AnalysisCommandResult {
  resultId: string;
  identity: HostGeneratedIdentity;
  resolution: TeamBuildResolution;
}

export interface AnalysisProgressEvent {
  analysisRunId: string;
  status: AnalysisStatus;
  characterId: string | null;
  characterStage: string | null;
  detail: string;
  error: string | null;
}

function isTauriRuntime() {
  return Boolean(window.__TAURI_INTERNALS__);
}

export function buildAnalysisInput(draft: PartyDraft, catalog: Catalog): AnalysisInput {
  const partyId = draft.partyId ?? draft.id;
  if (!partyId) throw new Error("編成IDがありません。");
  const members = draft.members.map((member) => {
    if (!member.characterId || !member.weaponId) {
      throw new Error(`スロット${member.slotIndex + 1}のキャラクターと武器が未選択です。`);
    }
    return {
      slotIndex: member.slotIndex,
      characterId: member.characterId,
      weaponId: member.weaponId,
      refinement: member.refinement,
      constellation: member.constellation,
      intent: {
        role: member.role,
        reactionOwnership: member.reactionOwnership,
        energyPriority: member.energyPriority,
        survivabilityPriority: member.survivabilityPriority,
      },
    };
  }) as AnalysisInput["members"];

  return {
    partyId,
    partyName: draft.name,
    gameVersion: catalog.gameVersion,
    members,
    assumptions: {
      characterLevel: 90,
      weaponLevel: 90,
      artifactLevel: 20,
      artifactRarity: 5,
      sheetTiming: "pre_combat",
      finalAscension: true,
      allTalentsAvailable: true,
      witchTeachingWhenApplicable: true,
    },
    versions: {
      catalogVersion: catalog.schemaVersion,
      sourcePolicyVersion: "source-policy-v1",
      promptVersion: "prompt-v1",
      schemaVersion: "character-research-v1",
      reconcilerVersion: "reconciler-v1",
      solverVersion: "solver-v1",
    },
  };
}

export async function subscribeAnalysisProgress(
  onProgress: (event: AnalysisProgressEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => undefined;
  return listen<AnalysisProgressEvent>("analysis-progress", ({ payload }) => onProgress(payload));
}

export async function startAnalysis(input: AnalysisInput): Promise<AnalysisCommandResult> {
  if (!isTauriRuntime()) {
    throw new Error("実Web調査はデスクトップアプリから実行してください。");
  }
  return invoke<AnalysisCommandResult>("start_analysis", { input });
}
