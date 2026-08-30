import { describe, expect, it } from "vitest";
import type { Catalog } from "../../domain/catalogTypes";
import { createEmptyParty } from "../party";
import { buildAnalysisInput } from "./analysisRepository";

const catalog: Catalog = {
  schemaVersion: "catalog-v2",
  gameVersion: "7.0",
  catalogUpdatedAt: "2026-08-30",
  characters: [],
  weapons: [],
  artifactSets: [],
};

describe("分析コマンド境界", () => {
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
      promptVersion: "prompt-v2",
      schemaVersion: "character-research-v1",
      reconcilerVersion: "reconciler-v2",
      solverVersion: "solver-v1",
    });
  });

  it("未選択スロットを分析入力へ変換しない", () => {
    const draft = createEmptyParty("party-empty");
    draft.name = "未完成";

    expect(() => buildAnalysisInput(draft, catalog)).toThrow("スロット1");
  });
});
