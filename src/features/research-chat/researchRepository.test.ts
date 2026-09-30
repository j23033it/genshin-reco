import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { researchRepository } from "./researchRepository";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("オンデマンド調査のTauri契約", () => {
  it("コマンド名とcamelCase引数を渡す", async () => {
    await researchRepository.sendMessage("条件", "s1");
    const members = [{ slotIndex: 0, name: "テスト", weapon: null }];
    await researchRepository.updateConditions("s1", members, "新しい編成");
    await researchRepository.startResearch("s1");
    await researchRepository.cancelResearch("s1");
    await researchRepository.listTeams();
    await researchRepository.loadTeam("t1");
    await researchRepository.renameTeam("t1", "変更後");
    await researchRepository.loadConversation("s1");
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["send_on_demand_message", { sessionId: "s1", message: "条件", game: "genshin" }],
      ["update_on_demand_conditions", { sessionId: "s1", members, title: "新しい編成" }],
      ["start_on_demand_research", { sessionId: "s1" }],
      ["cancel_on_demand_research", { sessionId: "s1" }],
      ["list_researched_teams", { game: "genshin" }],
      ["load_researched_team", { teamId: "t1" }],
      ["rename_researched_team", { teamId: "t1", title: "変更後" }],
      ["load_on_demand_conversation", { sessionId: "s1" }],
    ]);
  });

  it("進捗イベントのpayloadと購読解除をそのまま引き渡す", async () => {
    const callback = vi.fn();
    const stop = vi.fn();
    vi.mocked(listen).mockResolvedValueOnce(stop);
    const unsubscribe = await researchRepository.subscribeProgress(callback);
    expect(listen).toHaveBeenCalledWith(
      "on-demand-research-progress",
      expect.any(Function),
    );
    const payload = { sessionId: "s1", stage: "weapons", detail: "武器を確認" };
    vi.mocked(listen).mock.calls[0][1]({
      event: "on-demand-research-progress",
      id: 1,
      payload,
    });
    expect(callback).toHaveBeenCalledWith(payload);
    unsubscribe();
    expect(stop).toHaveBeenCalledOnce();
  });
});
