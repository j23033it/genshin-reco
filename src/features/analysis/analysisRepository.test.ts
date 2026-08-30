import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Catalog } from "../../domain/catalogTypes";
import { createEmptyParty } from "../party";
import { buildAnalysisInput, startAnalysis } from "./analysisRepository";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const catalog: Catalog = {
  schemaVersion: "catalog-v2",
  gameVersion: "7.0",
  catalogUpdatedAt: "2026-08-30",
  characters: [],
  weapons: [],
  artifactSets: [],
};

describe("分析コマンド境界", () => {
  beforeEach(() => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    invokeMock.mockReset().mockResolvedValue({
      resultId: "result-1",
      identity: {},
      resolution: {},
    });
  });

  afterEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("編成下書きを固定前提付きの分析入力へ変換する", () => {
    const draft = createEmptyParty("party-analysis");
    draft.name = "分析編成";
    draft.members = draft.members.map((member) => ({
      ...member,
      characterId: `character-${member.slotIndex}`,
      weaponId: `weapon-${member.slotIndex}`,
    })) as typeof draft.members;

    const input = buildAnalysisInput(draft, catalog);

    expect(input.partyId).toBe("party-analysis");
    expect(input.gameVersion).toBe("7.0");
    expect(input.members).toHaveLength(4);
    expect(input.members[0]).not.toHaveProperty("intent");
    expect(input.assumptions).toEqual({
      characterLevel: 90,
      weaponLevel: 90,
      artifactLevel: 20,
      artifactRarity: 5,
      sheetTiming: "pre_combat",
      finalAscension: true,
      allTalentsAvailable: true,
      witchTeachingWhenApplicable: true,
    });
    expect(input.versions).toEqual({
      catalogVersion: "catalog-v2",
      sourcePolicyVersion: "source-policy-v1",
      promptVersion: "prompt-v6",
      schemaVersion: "character-research-v2",
      reconcilerVersion: "reconciler-v5",
      solverVersion: "solver-v2",
    });
  });

  it("未選択スロットを分析入力へ変換しない", () => {
    const draft = createEmptyParty("party-empty");
    draft.name = "未完成";

    expect(() => buildAnalysisInput(draft, catalog)).toThrow("スロット1");
  });

  it("通常モードをTauriへ渡す", async () => {
    const draft = createEmptyParty("party-normal");
    draft.name = "通常分析";
    draft.members = draft.members.map((member) => ({
      ...member,
      characterId: `character-${member.slotIndex}`,
      weaponId: `weapon-${member.slotIndex}`,
    })) as typeof draft.members;
    const input = buildAnalysisInput(draft, catalog);

    await startAnalysis(input, "normal");

    expect(invokeMock).toHaveBeenCalledWith("start_analysis", { input, mode: "normal" });
  });

  it("高速モードをTauriへ渡す", async () => {
    const draft = createEmptyParty("party-fast");
    draft.name = "高速分析";
    draft.members = draft.members.map((member) => ({
      ...member,
      characterId: `character-${member.slotIndex}`,
      weaponId: `weapon-${member.slotIndex}`,
    })) as typeof draft.members;
    const input = buildAnalysisInput(draft, catalog);

    await startAnalysis(input, "fast");

    expect(invokeMock).toHaveBeenCalledWith("start_analysis", { input, mode: "fast" });
  });
});
