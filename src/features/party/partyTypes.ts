import type {
  BuildIntent,
  EnergyPriority,
  ReactionOwnership,
  SurvivabilityPriority,
} from "../../domain/analysisTypes";

export type SlotIndex = 0 | 1 | 2 | 3;
export type Constellation = 0 | 1 | 2 | 3 | 4 | 5 | 6;
export type Refinement = 1 | 2 | 3 | 4 | 5;

export interface PartyMemberDraft {
  slotIndex: SlotIndex;
  characterId: string | null;
  weaponId: string | null;
  constellation: Constellation;
  refinement: Refinement;
  role: BuildIntent;
  reactionOwnership: ReactionOwnership;
  energyPriority: EnergyPriority;
  survivabilityPriority: SurvivabilityPriority;
}

export interface PartyDraft {
  /** 保存済みデータとの対応に使うID。新規下書きでは未確定でもよい。 */
  partyId?: string;
  /** UIや外部呼び出し側が id を使う場合の互換フィールド。 */
  id?: string;
  name: string;
  members: [PartyMemberDraft, PartyMemberDraft, PartyMemberDraft, PartyMemberDraft];
}

export const ROLE_OPTIONS: ReadonlyArray<{ value: BuildIntent; label: string }> = [
  { value: "auto", label: "自動" },
  { value: "on_field_damage", label: "表で火力" },
  { value: "off_field_damage", label: "裏で火力" },
  { value: "reaction_trigger", label: "反応起点" },
  { value: "support", label: "サポート" },
  { value: "sustain", label: "耐久・回復" },
];

export const REACTION_OWNERSHIP_OPTIONS: ReadonlyArray<{
  value: ReactionOwnership;
  label: string;
}> = [
  { value: "unknown", label: "未指定" },
  { value: "main", label: "主に担当" },
  { value: "partial", label: "一部担当" },
  { value: "none", label: "担当しない" },
];

export const ENERGY_PRIORITY_OPTIONS: ReadonlyArray<{
  value: EnergyPriority;
  label: string;
}> = [
  { value: "balanced", label: "バランス" },
  { value: "damage_first", label: "火力優先" },
  { value: "burst_stability", label: "爆発安定" },
];

export const SURVIVABILITY_PRIORITY_OPTIONS: ReadonlyArray<{
  value: SurvivabilityPriority;
  label: string;
}> = [
  { value: "normal", label: "通常" },
  { value: "high", label: "高め" },
];

export const BUILD_INTENTS = ROLE_OPTIONS.map(({ value }) => value);
export const REACTION_OWNERSHIPS = REACTION_OWNERSHIP_OPTIONS.map(({ value }) => value);
export const ENERGY_PRIORITIES = ENERGY_PRIORITY_OPTIONS.map(({ value }) => value);
export const SURVIVABILITY_PRIORITIES = SURVIVABILITY_PRIORITY_OPTIONS.map(({ value }) => value);
