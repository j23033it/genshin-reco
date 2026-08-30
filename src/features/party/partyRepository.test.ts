import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createEmptyParty } from "./partyDraft";
import { listPartyDrafts, loadPartyDraft, savePartyDraft } from "./partyRepository";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe("編成DBクライアント", () => {
  afterEach(() => {
    vi.clearAllMocks();
    delete window.__TAURI_INTERNALS__;
  });

  it("ユーザー推測のビルド方針を含めずに保存する", async () => {
    window.__TAURI_INTERNALS__ = {};
    invokeMock.mockResolvedValue(undefined);
    const draft = createEmptyParty("party-1");
    draft.name = "変換検証";

    await savePartyDraft(draft);

    expect(invokeMock).toHaveBeenCalledWith("save_party_draft", {
      draft: expect.objectContaining({
        partyId: "party-1",
        members: expect.arrayContaining([
          expect.objectContaining({
            slotIndex: 0,
            constellation: 0,
            refinement: 1,
          }),
        ]),
      }),
    });
    const saved = invokeMock.mock.calls[0]?.[1] as { draft: { members: unknown[] } };
    expect(saved.draft.members[0]).not.toHaveProperty("intent");
  });

  it("旧保存データのintentを編集入力へ戻さない", async () => {
    window.__TAURI_INTERNALS__ = {};
    invokeMock.mockResolvedValue({
      partyId: "party-1",
      name: "読込検証",
      members: Array.from({ length: 4 }, (_, slotIndex) => ({
        slotIndex,
        characterId: null,
        weaponId: null,
        refinement: 1,
        constellation: 0,
        intent: {
          role: "auto",
          reactionOwnership: "unknown",
          energyPriority: "balanced",
          survivabilityPriority: "normal",
        },
      })),
    });

    const draft = await loadPartyDraft("party-1");

    expect(invokeMock).toHaveBeenCalledWith("load_party_draft", { partyId: "party-1" });
    expect(draft).toMatchObject({ partyId: "party-1", id: "party-1", name: "読込検証" });
    expect(draft?.members[0]).not.toHaveProperty("intent");
    expect(draft?.members[0]).not.toHaveProperty("role");
  });

  it("一覧はスナップショット本体ではなく検索用要約として取得する", async () => {
    window.__TAURI_INTERNALS__ = {};
    invokeMock.mockResolvedValue([
      { partyId: "party-1", name: "一覧検証", currentResultId: null, updatedAt: "2026-08-30" },
    ]);

    const summaries = await listPartyDrafts();

    expect(invokeMock).toHaveBeenCalledWith("list_party_drafts");
    expect(summaries[0].name).toBe("一覧検証");
  });
});
