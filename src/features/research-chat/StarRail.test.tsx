import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@testing-library/jest-dom/vitest";
import catalogData from "../../../public/data/star-rail/catalog.json";
import type { StarRailCatalog } from "../../domain/catalogTypes";
import { ResearchConditionsEditor } from "./ResearchConditionsEditor";
import { ResearchChatPrototype } from "./ResearchChatPrototype";
import { createDemoResearchRepository } from "./demoRepository";
import { validateRelicInput } from "./validateRelics";
import type { ResearchConversation } from "./types";

const catalog = catalogData as StarRailCatalog;
vi.mock("../catalog/loadCatalog", () => ({
  loadStarRailCatalog: vi.fn(async () => catalogData),
  loadCatalog: vi.fn(async () => ({ characters: [], weapons: [] })),
}));
vi.mock("./AppUpdateControl", () => ({ AppUpdateControl: () => null }));
afterEach(cleanup);
const conversation: ResearchConversation = {
  game: "star_rail", sessionId: "hsr", status: "ready", messages: [],
  members: ["ホタル", "ルアン・メェイ", "開拓者・調和", "ギャラガー"].map((name, slotIndex) => ({ name, slotIndex, weapon: null, refinement: null, constellation: null, relics: null })),
  createdAt: "created", updatedAt: "updated",
};

describe("スターレイルの入力と復元", () => {
  it("最新の愉悦編成で対応する光円錐と4.6遺物を選び固定条件を送信できる", async () => {
    const user = userEvent.setup(); const research = vi.fn();
    const members = ["パール", "緋英", "爻光", "ロビン・夏空の歌"].map((name, slotIndex) => ({ name, slotIndex, weapon: null, refinement: null, constellation: null, relics: null }));
    render(<ResearchConditionsEditor conversation={{ ...conversation, members }} disabled={false} onResearch={research} />);
    const pearl = within(screen.getByRole("group", { name: "パールの条件" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "この条件で調査する" })).toBeEnabled());
    const cone = pearl.getByLabelText("光円錐");
    expect(within(cone).queryByRole("option", { name: "立ち上がり高らかな歌を" })).not.toBeInTheDocument();
    await user.type(pearl.getByRole("searchbox", { name: "光円錐を検索" }), "明日に");
    await user.selectOptions(cone, "明日に捧げる色");
    await user.selectOptions(pearl.getByLabelText("重畳"), "3");
    await user.click(pearl.getByRole("button", { name: "パール：2凸" }));
    await user.selectOptions(pearl.getByLabelText("トンネル遺物"), "four_piece");
    await user.selectOptions(pearl.getByLabelText("4セット名"), "星灯す夢芝居の伶人");
    await user.selectOptions(pearl.getByLabelText("オーナメント（2セット）"), "宇宙生命科学研究院");
    await user.click(screen.getByRole("button", { name: "この条件で調査する" }));
    expect(research.mock.lastCall?.[0][0]).toMatchObject({ name: "パール", weapon: "明日に捧げる色", refinement: 3, constellation: 2, relics: { tunnel: { kind: "four_piece", set: "星灯す夢芝居の伶人" }, ornament: "宇宙生命科学研究院" } });
    expect(research.mock.lastCall?.[0].map((member: { name: string }) => member.name)).toEqual(members.map(member => member.name));
  });

  it("実在対象不明の保存済み光円錐とオーナメントを自動置換せず再選択を求める", async () => {
    const user = userEvent.setup(); const research = vi.fn(); const changed = vi.fn();
    const members = structuredClone(conversation.members);
    members[0].name = "開拓者・記憶";
    members[0].weapon = "瞬間を記憶に留めて";
    members[0].refinement = 3;
    members[0].relics = { tunnel: null, ornament: "悠久の地アムフォレウス" };
    render(<ResearchConditionsEditor conversation={{ ...conversation, members }} disabled={false} revision onResearch={research} onDraftChange={changed} />);
    const member = within(screen.getByRole("group", { name: "開拓者・記憶の条件" }));
    const submit = screen.getByRole("button", { name: "この条件で再調査する" });
    await screen.findByText("悠久の地アムフォレウス（未登録・要確認）");
    expect(submit).toBeDisabled();
    expect(changed).not.toHaveBeenCalled();
    expect(screen.getAllByRole("alert").map(e => e.textContent).join(" ")).toContain("実在対象を確認できません");
    expect(member.getByLabelText("オーナメント（2セット）")).toHaveValue("悠久の地アムフォレウス");
    expect(within(member.getByLabelText("光円錐")).queryByRole("option", { name: "瞬間を記憶に留めて" })).not.toBeInTheDocument();
    await user.selectOptions(member.getByLabelText("光円錐"), "瞬刻の勝機");
    expect(submit).toBeDisabled();
    await user.selectOptions(member.getByLabelText("オーナメント（2セット）"), "深慮に浸る巨樹");
    await user.click(submit);
    expect(research.mock.lastCall?.[0][0]).toMatchObject({ weapon: "瞬刻の勝機", refinement: null, relics: { ornament: "深慮に浸る巨樹" } });
  });

  it("条件の下書きを残して編成名を変更しても再調査で保存名を巻き戻さない", async () => {
    const user = userEvent.setup();
    render(<ResearchChatPrototype repository={createDemoResearchRepository()} />);
    await user.click(screen.getByRole("button", { name: "崩壊：スターレイル" }));
    await user.type(screen.getByRole("textbox", { name: "調べたい編成" }), "4人{Enter}");
    await user.click(await screen.findByRole("button", { name: "この条件で調査する" }));
    await screen.findByRole("heading", { name: "スターレイル編成（デモ）", level: 1 });
    await user.click(screen.getByRole("button", { name: "条件を変えて再調査" }));
    await user.click(await screen.findByRole("button", { name: "ホタル：1凸" }));
    await user.click(screen.getByRole("button", { name: "結果に戻る" }));
    await user.click(screen.getByRole("button", { name: "編成名を編集" }));
    const title = screen.getByRole("textbox", { name: "保存済みの編成名" });
    await user.clear(title);
    await user.type(title, "新しい保存名");
    await user.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByRole("heading", { name: "新しい保存名", level: 1 });
    await user.click(screen.getByRole("button", { name: "条件を変えて再調査" }));
    await user.click(await screen.findByRole("button", { name: "この条件で再調査する" }));
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent("新しい保存名");
  });

  it("未指定と0凸、片側指定、同名指定、2＋2途中を区別し、解除できる", async () => {
    const user = userEvent.setup(); const research = vi.fn();
    render(<ResearchConditionsEditor conversation={conversation} disabled={false} onResearch={research} />);
    const firefly = within(screen.getByRole("group", { name: "ホタルの条件" }));
    const ruanMei = within(screen.getByRole("group", { name: "ルアン・メェイの条件" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "この条件で調査する" })).toBeEnabled());
    await user.click(firefly.getByRole("button", { name: "ホタル：0凸" }));
    await user.selectOptions(firefly.getByLabelText("トンネル遺物"), "two_plus_two");
    await user.selectOptions(firefly.getByLabelText("2＋2セット名 1"), "草の穂ガンマン");
    expect(screen.getByRole("button", { name: "この条件で調査する" })).toBeDisabled();
    await user.selectOptions(firefly.getByLabelText("2＋2セット名 2"), "夢を弄ぶ時計屋");
    await user.selectOptions(firefly.getByLabelText("オーナメント（2セット）"), "折れた竜骨");
    await user.selectOptions(ruanMei.getByLabelText("オーナメント（2セット）"), "折れた竜骨");
    await user.click(screen.getByRole("button", { name: "この条件で調査する" }));
    const members = research.mock.calls[0][0];
    expect(members[0]).toMatchObject({ constellation: 0, relics: { tunnel: { kind: "two_plus_two", sets: ["草の穂ガンマン", "夢を弄ぶ時計屋"] }, ornament: "折れた竜骨" } });
    expect(members[1]).toMatchObject({ constellation: null, relics: { tunnel: null, ornament: "折れた竜骨" } });
    expect(members[2].relics).toBeNull();
    await user.selectOptions(firefly.getByLabelText("トンネル遺物"), "unspecified");
    await user.selectOptions(firefly.getByLabelText("オーナメント（2セット）"), "");
    await user.click(screen.getByRole("button", { name: "この条件で調査する" }));
    expect(research.mock.lastCall?.[0][0].relics).toEqual({ tunnel: null, ornament: null });
  });

  it("旧セット名とカテゴリ違いを消さず、復旧を説明する", async () => {
    const members = structuredClone(conversation.members);
    members[0].relics = { tunnel: { kind: "four_piece", set: "旧名称" }, ornament: "草の穂ガンマン" };
    render(<ResearchConditionsEditor conversation={{ ...conversation, members }} disabled={false} onResearch={vi.fn()} />);
    await screen.findByText("旧名称（未登録・要確認）");
    expect(screen.getByRole("button", { name: "この条件で調査する" })).toBeDisabled();
    expect(validateRelicInput(members[0], catalog)).toContain("未登録");
  });

  it("ゲーム往復でチャットの入力と未保存の固定条件を分離して維持する", async () => {
    const user = userEvent.setup(); const repository = createDemoResearchRepository();
    render(<ResearchChatPrototype repository={repository} />);
    const composer = () => screen.getByRole("textbox", { name: "調べたい編成" });
    await user.type(composer(), "原神の入力途中");
    await user.click(screen.getByRole("button", { name: "崩壊：スターレイル" }));
    expect(composer()).toHaveValue("");
    await user.type(composer(), "スターレイルの編成{Enter}");
    const firefly = within(await screen.findByRole("group", { name: "ホタルの条件" }));
    await user.selectOptions(firefly.getByLabelText("トンネル遺物"), "four_piece");
    await user.selectOptions(firefly.getByLabelText("4セット名"), "草の穂ガンマン");
    await user.click(screen.getByRole("button", { name: "原神" }));
    expect(composer()).toHaveValue("原神の入力途中");
    expect(screen.queryByRole("group", { name: "ホタルの条件" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "崩壊：スターレイル" }));
    const restored = within(await screen.findByRole("group", { name: "ホタルの条件" }));
    expect(restored.getByLabelText("4セット名")).toHaveValue("草の穂ガンマン");
    await user.click(await screen.findByRole("button", { name: "この条件で調査する" }));
    await screen.findByText("スターレイル編成（デモ）", { selector: "h1" });
    expect(screen.getAllByText("指定を維持")).toHaveLength(1);
    expect(screen.getAllByText("自動提案")).toHaveLength(7);
    await user.click(screen.getByRole("button", { name: "条件を変えて再調査" }));
    const gallagher = within(await screen.findByRole("group", { name: "ギャラガーの条件" }));
    expect(gallagher.getByLabelText("トンネル遺物")).toHaveValue("unspecified");
    expect(gallagher.getByLabelText("オーナメント（2セット）")).toHaveValue("");
  });
});
