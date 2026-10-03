import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import genshin from "../../../public/data/catalog.json";
import starRail from "../../../public/data/star-rail/catalog.json";
import { loadCatalog, loadStarRailCatalog } from "../catalog/loadCatalog";
import { ResearchConditionsEditor } from "./ResearchConditionsEditor";
import type { ResearchConversation } from "./types";

vi.mock("../catalog/loadCatalog", () => ({ loadCatalog: vi.fn(), loadStarRailCatalog: vi.fn() }));
afterEach(cleanup);
beforeEach(() => {
  vi.mocked(loadCatalog).mockResolvedValue(genshin as Awaited<ReturnType<typeof loadCatalog>>);
  vi.mocked(loadStarRailCatalog).mockResolvedValue(starRail as Awaited<ReturnType<typeof loadStarRailCatalog>>);
});

const cases = [
  { game: "genshin" as const, character: "アルレッキーノ", label: "武器", selected: "赤月のシルエット", other: "和璞鳶", incompatible: "若水" },
  { game: "star_rail" as const, character: "ホタル", label: "光円錐", selected: "夢が帰り着く場所", other: "とある星神の殞落を記す", incompatible: "鏡の中の私" },
];
function conversation(game: "genshin" | "star_rail", name: string, weapon: string | null, refinement: number | null = 3): ResearchConversation {
  return { game, sessionId: "test", status: "ready", messages: [], members: [{ slotIndex: 0, name, weapon, refinement }], createdAt: "created", updatedAt: "updated" };
}

for (const item of cases) {
  describe(`${item.game}の装備条件`, () => {
    it.each([false, true])("新規・再調査(revision=%s)で分類を守り、検索だけでは装備と精錬・重畳を変更しない", async revision => {
      const user = userEvent.setup(); const onResearch = vi.fn(); const onDraftChange = vi.fn();
      render(<ResearchConditionsEditor conversation={conversation(item.game, item.character, item.selected)} disabled={false} revision={revision} onResearch={onResearch} onDraftChange={onDraftChange} />);
      const select = screen.getByRole("combobox", { name: item.label });
      await waitFor(() => expect(select).toBeEnabled());
      expect(select).toHaveValue(item.selected);
      expect(within(select).queryByRole("option", { name: item.incompatible })).not.toBeInTheDocument();
      const search = screen.getByRole("searchbox", { name: `${item.label}を検索` });
      await user.type(search, item.incompatible);
      expect(screen.getByRole("status")).toHaveTextContent(`一致する${item.label}はありません。`);
      expect(select).toHaveValue(item.selected);
      expect(onDraftChange).not.toHaveBeenCalled();
      await user.click(screen.getByRole("button", { name: revision ? "この条件で再調査する" : "この条件で調査する" }));
      expect(onResearch.mock.lastCall?.[0][0]).toMatchObject({ weapon: item.selected, refinement: 3 });
      await user.clear(search);
      await user.type(search, item.other.slice(0, 3));
      await user.selectOptions(select, item.other);
      expect(onDraftChange.mock.lastCall?.[0].members[0]).toMatchObject({ weapon: item.other, refinement: null });
      await user.click(screen.getByRole("button", { name: `${item.label}の検索をクリア` }));
      expect(within(select).queryByRole("option", { name: item.incompatible })).not.toBeInTheDocument();
    });

    it.each(["incompatible", "unknown"])("保存済みの%s装備を候補にせず、選び直すまで調査を止める", async kind => {
      const user = userEvent.setup(); const onResearch = vi.fn();
      const weapon = kind === "incompatible" ? item.incompatible : "未登録装備";
      render(<ResearchConditionsEditor conversation={conversation(item.game, item.character, weapon)} disabled={false} revision onResearch={onResearch} />);
      const select = screen.getByRole("combobox", { name: item.label });
      await waitFor(() => expect(select).toBeEnabled());
      expect(within(select).queryByRole("option", { name: weapon })).not.toBeInTheDocument();
      expect(within(select).getByRole("option", { name: "装備を選び直してください" })).toBeDisabled();
      expect(select).toHaveAttribute("aria-invalid", "true");
      expect(screen.getByRole("alert")).toHaveTextContent(weapon);
      const submit = screen.getByRole("button", { name: "この条件で再調査する" });
      expect(submit).toBeDisabled();
      await user.selectOptions(select, "");
      expect(submit).toBeEnabled();
      await user.click(submit);
      expect(onResearch.mock.lastCall?.[0][0]).toMatchObject({ weapon: null, refinement: null });
    });

    it("キャラクターの分類が不明なら全装備を候補にせず指定なしにできる", async () => {
      const user = userEvent.setup();
      render(<ResearchConditionsEditor conversation={conversation(item.game, "未登録キャラ", item.selected)} disabled={false} onResearch={vi.fn()} />);
      const select = screen.getByRole("combobox", { name: item.label });
      await waitFor(() => expect(select).toBeEnabled());
      expect(screen.getByRole("searchbox")).toBeDisabled();
      expect(within(select).getAllByRole("option")).toHaveLength(2);
      expect(screen.getByText(/カタログで確認できないため、候補を表示できません/)).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "この条件で調査する" })).toBeDisabled();
      await user.selectOptions(select, "");
      expect(within(select).getAllByRole("option")).toHaveLength(1);
      const submit = screen.getByRole("button", { name: "この条件で調査する" });
      if (item.game === "star_rail") expect(submit).toBeDisabled();
      else expect(submit).toBeEnabled();
    });

    it("キャラの分類データが空なら同じ空分類の装備も候補にしない", async () => {
      if (item.game === "genshin") {
        const catalog = structuredClone(genshin) as Awaited<ReturnType<typeof loadCatalog>>;
        const character = catalog.characters.find(entry => entry.name === item.character)!;
        character.weaponType = "";
        catalog.weapons.find(entry => entry.name === item.selected)!.weaponType = "";
        vi.mocked(loadCatalog).mockResolvedValueOnce(catalog);
      } else {
        const catalog = structuredClone(starRail) as Awaited<ReturnType<typeof loadStarRailCatalog>>;
        catalog.characters.find(entry => entry.name === item.character)!.path = "";
        catalog.lightCones.find(entry => entry.name === item.selected)!.path = "";
        vi.mocked(loadStarRailCatalog).mockResolvedValueOnce(catalog);
      }
      render(<ResearchConditionsEditor conversation={conversation(item.game, item.character, item.selected)} disabled={false} onResearch={vi.fn()} />);
      const select = screen.getByRole("combobox", { name: item.label });
      await waitFor(() => expect(select).toBeEnabled());
      expect(screen.getByRole("searchbox")).toBeDisabled();
      expect(within(select).queryByRole("option", { name: item.selected })).not.toBeInTheDocument();
      expect(within(select).getAllByRole("option")).toHaveLength(2);
      expect(screen.getByRole("button", { name: "この条件で調査する" })).toBeDisabled();
    });

    it("カタログ読み込み失敗時に自由入力で制約を回避させない", async () => {
      vi.mocked(item.game === "genshin" ? loadCatalog : loadStarRailCatalog).mockRejectedValueOnce(new Error("offline"));
      render(<ResearchConditionsEditor conversation={conversation(item.game, item.character, item.selected)} disabled={false} onResearch={vi.fn()} />);
      await screen.findByText(/カタログを読み込めません/);
      expect(screen.getByRole("searchbox")).toBeDisabled();
      expect(screen.getByRole("button", { name: "この条件で調査する" })).toBeDisabled();
      expect(screen.getByRole("combobox", { name: item.label })).toHaveValue(item.selected);
    });
  });
}
