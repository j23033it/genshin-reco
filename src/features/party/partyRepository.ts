import { invoke } from "@tauri-apps/api/core";
import type { PartyDraft, PartyMemberDraft } from "./partyTypes";

export interface PartySummary {
  partyId: string;
  name: string;
  currentResultId: string | null;
  updatedAt: string;
}

interface DatabasePartyMemberDraft {
  slotIndex: number;
  characterId: string | null;
  weaponId: string | null;
  refinement: number;
  constellation: number;
}

interface DatabasePartyDraft {
  partyId: string;
  name: string;
  members: DatabasePartyMemberDraft[];
}

const browserDrafts = new Map<string, PartyDraft>();

function isTauriRuntime() {
  return Boolean(window.__TAURI_INTERNALS__);
}

function toDatabaseDraft(draft: PartyDraft): DatabasePartyDraft {
  const partyId = draft.partyId ?? draft.id;
  if (!partyId) throw new Error("編成IDがありません。");
  return {
    partyId,
    name: draft.name,
    members: draft.members.map((member) => ({
      slotIndex: member.slotIndex,
      characterId: member.characterId,
      weaponId: member.weaponId,
      refinement: member.refinement,
      constellation: member.constellation,
    })),
  };
}

function fromDatabaseDraft(draft: DatabasePartyDraft): PartyDraft {
  const members = draft.members.map(
    (member): PartyMemberDraft => ({
      slotIndex: member.slotIndex as PartyMemberDraft["slotIndex"],
      characterId: member.characterId,
      weaponId: member.weaponId,
      refinement: member.refinement as PartyMemberDraft["refinement"],
      constellation: member.constellation as PartyMemberDraft["constellation"],
    }),
  ) as PartyDraft["members"];
  return { partyId: draft.partyId, id: draft.partyId, name: draft.name, members };
}

export async function savePartyDraft(draft: PartyDraft): Promise<void> {
  if (isTauriRuntime()) {
    await invoke("save_party_draft", { draft: toDatabaseDraft(draft) });
    return;
  }
  const copy = structuredClone(draft);
  const partyId = copy.partyId ?? copy.id;
  if (!partyId) throw new Error("編成IDがありません。");
  browserDrafts.set(partyId, { ...copy, partyId, id: partyId });
}

export async function loadPartyDraft(partyId: string): Promise<PartyDraft | null> {
  if (isTauriRuntime()) {
    const draft = await invoke<DatabasePartyDraft | null>("load_party_draft", { partyId });
    return draft ? fromDatabaseDraft(draft) : null;
  }
  return structuredClone(browserDrafts.get(partyId) ?? null);
}

export async function listPartyDrafts(): Promise<PartySummary[]> {
  if (isTauriRuntime()) {
    return invoke<PartySummary[]>("list_party_drafts");
  }
  return Array.from(browserDrafts.values()).map((draft) => ({
    partyId: draft.partyId ?? draft.id ?? "",
    name: draft.name,
    currentResultId: null,
    updatedAt: "",
  }));
}

export async function deletePartyDraft(partyId: string): Promise<void> {
  if (isTauriRuntime()) {
    await invoke("delete_party_draft", { partyId });
    return;
  }
  browserDrafts.delete(partyId);
}
