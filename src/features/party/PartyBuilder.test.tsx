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
      onAnalyze={vi.fn()}
    />,
  );
}

describe("PartyBuilder", () => {
  it("空の下書きは分析できず、保存ボタンを表示しない", () => {
    const draft = createEmptyParty("empty-party");
    renderBuilder(draft);

    expect(screen.getByLabelText("編成名")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "分析を開始" })).toBeDisabled();
    expect(screen.queryByLabelText("役割")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("反応担当")).not.toBeInTheDocument();
    expect(screen.getByText(/ビルド方針は、編成と検証済みの根拠から分析時に判断します/)).toBeInTheDocument();
  });

  it("名前付きの途中下書きも、4人が揃うまで分析できない", () => {
    const draft = createEmptyParty("partial-party");
    draft.name = "途中の編成";
    draft.members[0] = { ...draft.members[0], characterId: "one-hand" };
    renderBuilder(draft);

    expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "分析を開始" })).toBeDisabled();
    expect(screen.getAllByText(/キャラクターを選択してください/).length).toBeGreaterThan(0);
  });

  it("武器は選択キャラクターと同じ武器種だけに絞られる", async () => {
    const user = userEvent.setup();
    function ControlledBuilder() {
      const [draft, setDraft] = useState(createEmptyParty("filter-party"));
      return <PartyBuilder catalog={catalog} draft={draft} onChange={setDraft} onAnalyze={vi.fn()} />;
    }
    render(<ControlledBuilder />);
    const characterSelect = screen.getAllByLabelText("キャラクター")[0];
    await user.selectOptions(characterSelect, "one-hand");

    const weaponSelect = screen.getAllByLabelText("武器")[0];
    expect(weaponSelect).toHaveValue("");
    expect(weaponSelect.querySelector('option[value="sword"]')).toBeInTheDocument();
    expect(weaponSelect.querySelector('option[value="bow-weapon"]')).not.toBeInTheDocument();
  });

  it("名前・元素・武器種でキャラクター候補を検索できる", async () => {
    const user = userEvent.setup();
    function ControlledBuilder() {
      const [draft, setDraft] = useState(createEmptyParty("search-party"));
      return <PartyBuilder catalog={catalog} draft={draft} onChange={setDraft} onAnalyze={vi.fn()} />;
    }
    render(<ControlledBuilder />);

    const searchInput = screen.getAllByLabelText("キャラクターを検索")[0];
    const characterSelect = screen.getAllByLabelText("キャラクター")[0];
    await user.type(searchInput, "雷");

    expect(characterSelect.querySelector('option[value="bow"]')).toBeInTheDocument();
    expect(characterSelect.querySelector('option[value="one-hand"]')).not.toBeInTheDocument();
    expect(screen.getByText("1人が一致しました。")).toBeInTheDocument();

    await user.selectOptions(characterSelect, "bow");
    await user.clear(searchInput);
    await user.type(searchInput, "存在しない名前");

    expect(characterSelect).toHaveValue("bow");
    expect(characterSelect.querySelector('option[value="bow"]')).toBeInTheDocument();
    expect(screen.getByText("0人が一致しました。")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "一致するキャラクターはいません" })).toBeDisabled();
  });

  it("新しい下書きIDを受け取ると各スロットのキャラクター検索を初期化する", async () => {
    const user = userEvent.setup();
    function ControlledBuilder() {
      const [draft, setDraft] = useState(createEmptyParty("first-party"));
      return (
        <>
          <button type="button" onClick={() => setDraft(createEmptyParty("second-party"))}>
            新しい下書き
          </button>
          <PartyBuilder catalog={catalog} draft={draft} onChange={setDraft} onAnalyze={vi.fn()} />
        </>
      );
    }
    render(<ControlledBuilder />);

    const searchInput = screen.getAllByLabelText("キャラクターを検索")[0];
    await user.type(searchInput, "雷");
    expect(searchInput).toHaveValue("雷");

    await user.click(screen.getByRole("button", { name: "新しい下書き" }));
    expect(screen.getAllByLabelText("キャラクターを検索")[0]).toHaveValue("");
  });

  it("重複キャラクターと旅人variantの同居を分析エラーにする", () => {
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
    expect(screen.getByRole("button", { name: "分析を開始" })).toBeDisabled();
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

  it("分析可能な編成では分析だけを呼び出せる", async () => {
    const user = userEvent.setup();
    const onAnalyze = vi.fn();
    const draft = fullDraft();
    render(
      <PartyBuilder
        catalog={catalog}
        draft={draft}
        onChange={vi.fn()}
        onAnalyze={onAnalyze}
      />,
    );

    expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "分析を開始" })).toBeEnabled();
    expect(screen.getByRole("radio", { name: "通常" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "高速" })).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "分析を開始" }));
    expect(onAnalyze).toHaveBeenCalledWith(draft);
  });

  it("分析モードは通常を既定にし、高速へ変更しても下書きへ保存しない", async () => {
    const user = userEvent.setup();
    const onAnalyze = vi.fn();
    const draft = fullDraft();

    function ControlledBuilder() {
      const [analysisMode, setAnalysisMode] = useState<"normal" | "fast">("normal");
      return (
        <PartyBuilder
          catalog={catalog}
          draft={draft}
          onChange={vi.fn()}
          onAnalyze={onAnalyze}
          analysisMode={analysisMode}
          onAnalysisModeChange={setAnalysisMode}
        />
      );
    }

    render(<ControlledBuilder />);

    const normalMode = screen.getByRole("radio", { name: "通常" });
    const fastMode = screen.getByRole("radio", { name: "高速" });
    normalMode.focus();
    await user.keyboard("{ArrowRight}");

    expect(fastMode).toBeChecked();
    expect(normalMode).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "分析を開始" }));
    expect(onAnalyze).toHaveBeenCalledWith(draft);
    expect(onAnalyze.mock.calls[0]?.[0]).not.toHaveProperty("analysisMode");
  });

  it("分析中は分析モードを変更できない", () => {
    render(
      <PartyBuilder
        catalog={catalog}
        draft={fullDraft()}
        onChange={vi.fn()}
        onAnalyze={vi.fn()}
        disabled
      />,
    );

    expect(screen.getByRole("radio", { name: "通常" })).toBeDisabled();
    expect(screen.getByRole("radio", { name: "高速" })).toBeDisabled();
  });

  it("保存編成の編集時は分析更新ボタンを表示する", () => {
    render(
      <PartyBuilder
        catalog={catalog}
        draft={fullDraft()}
        onChange={vi.fn()}
        onAnalyze={vi.fn()}
        mode="edit"
      />,
    );

    expect(screen.getByRole("button", { name: "分析を更新" })).toBeEnabled();
    expect(screen.getByRole("heading", { name: "保存編成を編集" })).toBeInTheDocument();
  });

  it("可視ラベルの付いたネイティブ要素をキーボードで順に操作できる", async () => {
    const user = userEvent.setup();
    renderBuilder(fullDraft());
    const nameInput = screen.getByLabelText("編成名");
    const firstCharacterSearch = screen.getAllByLabelText("キャラクターを検索")[0];
    const firstCharacter = screen.getAllByLabelText("キャラクター")[0];

    nameInput.focus();
    await user.tab();
    expect(firstCharacterSearch).toHaveFocus();
    await user.tab();
    expect(firstCharacter).toHaveFocus();
  });
});
