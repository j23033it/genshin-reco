import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Catalog } from "../../domain/catalogTypes";
import { createEmptyParty } from "./partyDraft";
import { PartyBuilder } from "./PartyBuilder";
import type { PartyDraft } from "./partyTypes";

const catalog: Catalog = {
  schemaVersion: "catalog-v2",
  gameVersion: "7.0",
  catalogUpdatedAt: "2026-08-24",
  characters: [
    {
      id: "traveler-anemo",
      name: "旅人（風）",
      element: "風",
      weaponType: "片手剣",
      rarity: 5,
      imageUrl: "traveler.png",
    },
    {
      id: "traveler-geo",
      name: "旅人（岩）",
      element: "岩",
      weaponType: "片手剣",
      rarity: 5,
      imageUrl: "traveler.png",
    },
    { id: "one-hand", name: "片手剣キャラ", element: "水", weaponType: "片手剣", rarity: 5, imageUrl: "one.png" },
    { id: "bow", name: "弓キャラ", element: "雷", weaponType: "弓", rarity: 4, imageUrl: "bow.png" },
    { id: "claymore", name: "両手剣キャラ", element: "炎", weaponType: "両手剣", rarity: 5, imageUrl: "claymore.png" },
  ],
  weapons: [
    { id: "sword", name: "片手剣", weaponType: "片手剣", rarity: 5, imageUrl: "sword.png" },
    { id: "bow-weapon", name: "弓", weaponType: "弓", rarity: 4, imageUrl: "bow-weapon.png" },
    { id: "claymore-weapon", name: "両手剣", weaponType: "両手剣", rarity: 5, imageUrl: "claymore-weapon.png" },
  ],
  artifactSets: [],
};

function draftWithMembers(members: PartyDraft["members"]): PartyDraft {
  return { ...createEmptyParty("test-party"), name: "テスト編成", members };
}

function fullDraft(): PartyDraft {
  const empty = createEmptyParty("test-party");
  return draftWithMembers([
    { ...empty.members[0], characterId: "one-hand", weaponId: "sword" },
    { ...empty.members[1], characterId: "bow", weaponId: "bow-weapon" },
    { ...empty.members[2], characterId: "claymore", weaponId: "claymore-weapon" },
    { ...empty.members[3], characterId: "traveler-anemo", weaponId: "sword" },
  ]);
}

function renderBuilder(draft: PartyDraft, onChange = vi.fn()) {
  return render(
    <PartyBuilder
      catalog={catalog}
      draft={draft}
      onChange={onChange}
      onSave={vi.fn()}
      onAnalyze={vi.fn()}
    />,
  );
}

describe("PartyBuilder", () => {
  it("空の下書きは保存も分析もできない", () => {
    const draft = createEmptyParty("empty-party");
    renderBuilder(draft);

    expect(screen.getByLabelText("編成名")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "分析を開始" })).toBeDisabled();
  });

  it("名前付きの途中下書きは保存できるが分析できない", () => {
    const draft = createEmptyParty("partial-party");
    draft.name = "途中の編成";
    draft.members[0] = { ...draft.members[0], characterId: "one-hand" };
    renderBuilder(draft);

    expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "分析を開始" })).toBeDisabled();
    expect(screen.getAllByText(/キャラクターを選択してください/).length).toBeGreaterThan(0);
  });

  it("武器は選択キャラクターと同じ武器種だけに絞られる", async () => {
    const user = userEvent.setup();
    function ControlledBuilder() {
      const [draft, setDraft] = useState(createEmptyParty("filter-party"));
      return <PartyBuilder catalog={catalog} draft={draft} onChange={setDraft} onSave={vi.fn()} onAnalyze={vi.fn()} />;
    }
    render(<ControlledBuilder />);
    const characterSelect = screen.getAllByLabelText("キャラクター")[0];
    await user.selectOptions(characterSelect, "one-hand");

    const weaponSelect = screen.getAllByLabelText("武器")[0];
    expect(weaponSelect).toHaveValue("");
    expect(weaponSelect.querySelector('option[value="sword"]')).toBeInTheDocument();
    expect(weaponSelect.querySelector('option[value="bow-weapon"]')).not.toBeInTheDocument();
  });

  it("重複キャラクターと旅人variantの同居を保存エラーにする", () => {
    const empty = createEmptyParty("duplicate-party");
    const draft = draftWithMembers([
      { ...empty.members[0], characterId: "one-hand" },
      { ...empty.members[1], characterId: "one-hand" },
      { ...empty.members[2], characterId: "traveler-anemo" },
      { ...empty.members[3], characterId: "traveler-geo" },
    ]);
    renderBuilder(draft);

    expect(screen.getAllByText(/同じキャラクターが選択されています/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/旅人のvariantは同じ編成に複数入れられません/).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });

  it("同じ武器種のキャラクター変更では武器と精錬を保持し、異なる武器種では解除してR1に戻す", () => {
    const empty = createEmptyParty("change-party");
    const draft = draftWithMembers([
      { ...empty.members[0], characterId: "one-hand", weaponId: "sword", refinement: 4 },
      empty.members[1],
      empty.members[2],
      empty.members[3],
    ]);
    const onChange = vi.fn();
    renderBuilder(draft, onChange);
    const characterSelect = screen.getAllByLabelText("キャラクター")[0];

    fireEvent.change(characterSelect, { target: { value: "traveler-anemo" } });
    const retained = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0] as PartyDraft;
    expect(retained.members[0]).toMatchObject({ characterId: "traveler-anemo", weaponId: "sword", refinement: 4 });

    fireEvent.change(characterSelect, { target: { value: "claymore" } });
    const reset = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0] as PartyDraft;
    expect(reset.members[0]).toMatchObject({ characterId: "claymore", weaponId: null, refinement: 1 });
  });

  it("分析可能な編成では保存と分析を別々に呼び出せる", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    const onAnalyze = vi.fn();
    const draft = fullDraft();
    render(
      <PartyBuilder
        catalog={catalog}
        draft={draft}
        onChange={vi.fn()}
        onSave={onSave}
        onAnalyze={onAnalyze}
      />,
    );

    expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "分析を開始" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "保存" }));
    await user.click(screen.getByRole("button", { name: "分析を開始" }));
    expect(onSave).toHaveBeenCalledWith(draft);
    expect(onAnalyze).toHaveBeenCalledWith(draft);
  });

  it("可視ラベルの付いたネイティブ要素をキーボードで順に操作できる", async () => {
    const user = userEvent.setup();
    renderBuilder(fullDraft());
    const nameInput = screen.getByLabelText("編成名");
    const firstCharacter = screen.getAllByLabelText("キャラクター")[0];

    nameInput.focus();
    await user.tab();
    expect(firstCharacter).toHaveFocus();
  });
});
