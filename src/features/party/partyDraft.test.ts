import { describe, expect, it } from "vitest";
import type { Catalog } from "../../domain/catalogTypes";
import { createEmptyParty, validatePartyDraft } from "./partyDraft";
import type { PartyDraft } from "./partyTypes";

const catalog: Catalog = {
  schemaVersion: "catalog-v2",
  gameVersion: "7.0",
  catalogUpdatedAt: "2026-08-24",
  characters: [
    { id: "sword-user", name: "剣士", element: "炎", weaponType: "片手剣", rarity: 5, imageUrl: "sword.png" },
    { id: "bow-user", name: "弓使い", element: "雷", weaponType: "弓", rarity: 4, imageUrl: "bow.png" },
    { id: "claymore-user", name: "大剣使い", element: "岩", weaponType: "両手剣", rarity: 5, imageUrl: "claymore.png" },
    { id: "polearm-user", name: "槍使い", element: "水", weaponType: "長柄武器", rarity: 4, imageUrl: "polearm.png" },
  ],
  weapons: [
    { id: "sword", name: "剣", weaponType: "片手剣", rarity: 5, imageUrl: "sword-weapon.png" },
    { id: "bow", name: "弓", weaponType: "弓", rarity: 4, imageUrl: "bow-weapon.png" },
    { id: "claymore", name: "大剣", weaponType: "両手剣", rarity: 5, imageUrl: "claymore-weapon.png" },
    { id: "polearm", name: "槍", weaponType: "長柄武器", rarity: 4, imageUrl: "polearm-weapon.png" },
  ],
  artifactSets: [],
};

function namedParty(): PartyDraft {
  return { ...createEmptyParty("validation-party"), name: "検証用編成" };
}

function completeParty(): PartyDraft {
  const draft = namedParty();
  draft.members = [
    { ...draft.members[0], characterId: "sword-user", weaponId: "sword" },
    { ...draft.members[1], characterId: "bow-user", weaponId: "bow" },
    { ...draft.members[2], characterId: "claymore-user", weaponId: "claymore" },
    { ...draft.members[3], characterId: "polearm-user", weaponId: "polearm" },
  ];
  return draft;
}

describe("validatePartyDraft", () => {
  it("保存は名前・4スロット・値の範囲を検証する", () => {
    const draft = namedParty();
    draft.members[0] = { ...draft.members[0], constellation: 7 as PartyDraft["members"][0]["constellation"] };

    const result = validatePartyDraft(draft, catalog);

    expect(result.canSave).toBe(false);
    expect(result.saveErrors.some((validationError) => validationError.code === "constellation-range")).toBe(true);
  });

  it("4人分そろっていても武器種が違えば分析できない", () => {
    const draft = completeParty();
    draft.members[0] = { ...draft.members[0], weaponId: "bow" };

    const result = validatePartyDraft(draft, catalog);

    expect(result.canSave).toBe(true);
    expect(result.canAnalyze).toBe(false);
    expect(result.analyzeErrors.some((validationError) => validationError.code === "weapon-type-mismatch")).toBe(true);
  });

  it("4人分のキャラクターと武器が一致すれば保存と分析の両方が可能", () => {
    const result = validatePartyDraft(completeParty(), catalog);

    expect(result.canSave).toBe(true);
    expect(result.canAnalyze).toBe(true);
    expect(result.saveErrors).toHaveLength(0);
    expect(result.analyzeErrors).toHaveLength(0);
  });
});
