export type SlotIndex = 0 | 1 | 2 | 3;
export type Constellation = 0 | 1 | 2 | 3 | 4 | 5 | 6;
export type Refinement = 1 | 2 | 3 | 4 | 5;

export interface PartyMemberDraft {
  slotIndex: SlotIndex;
  characterId: string | null;
  weaponId: string | null;
  constellation: Constellation;
  refinement: Refinement;
}

export interface PartyDraft {
  /** 保存済みデータとの対応に使うID。新規下書きでは未確定でもよい。 */
  partyId?: string;
  /** UIや外部呼び出し側が id を使う場合の互換フィールド。 */
  id?: string;
  name: string;
  members: [PartyMemberDraft, PartyMemberDraft, PartyMemberDraft, PartyMemberDraft];
}
