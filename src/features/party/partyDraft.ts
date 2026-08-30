import type { Catalog, Character } from "../../domain/catalogTypes";
import type {
  BuildIntent,
  EnergyPriority,
  ReactionOwnership,
  SurvivabilityPriority,
} from "../../domain/analysisTypes";
import {
  BUILD_INTENTS,
  ENERGY_PRIORITIES,
  REACTION_OWNERSHIPS,
  SURVIVABILITY_PRIORITIES,
  type PartyDraft,
  type PartyMemberDraft,
  type SlotIndex,
} from "./partyTypes";

export const PARTY_SLOT_COUNT = 4;

const SLOT_INDICES: readonly SlotIndex[] = [0, 1, 2, 3];

export const DEFAULT_MEMBER_VALUES: Omit<PartyMemberDraft, "slotIndex"> = {
  characterId: null,
  weaponId: null,
  constellation: 0,
  refinement: 1,
  role: "auto",
  reactionOwnership: "unknown",
  energyPriority: "balanced",
  survivabilityPriority: "normal",
};

export function createEmptyParty(partyId = "new-party"): PartyDraft {
  const members = SLOT_INDICES.map((slotIndex) => ({
    slotIndex,
    ...DEFAULT_MEMBER_VALUES,
  })) as [PartyMemberDraft, PartyMemberDraft, PartyMemberDraft, PartyMemberDraft];

  return {
    partyId,
    id: partyId,
    name: "",
    members,
  };
}

export type PartyValidationErrorCode =
  | "name-required"
  | "name-too-long"
  | "members-structure"
  | "slot-index"
  | "constellation-range"
  | "refinement-range"
  | "role-invalid"
  | "reaction-ownership-invalid"
  | "energy-priority-invalid"
  | "survivability-priority-invalid"
  | "duplicate-character"
  | "traveler-variant-conflict"
  | "character-required"
  | "weapon-required"
  | "character-not-found"
  | "weapon-not-found"
  | "weapon-type-mismatch"
  | "catalog-required";

export interface PartyValidationError {
  code: PartyValidationErrorCode;
  message: string;
  field?: string;
  slotIndex?: number;
}

export interface PartyValidationResult {
  canSave: boolean;
  canAnalyze: boolean;
  saveErrors: PartyValidationError[];
  analyzeErrors: PartyValidationError[];
  /** 両方の操作で確認されたエラーを、表示順を保ってまとめたもの。 */
  errors: PartyValidationError[];
  save: {
    valid: boolean;
    errors: PartyValidationError[];
  };
  analyze: {
    valid: boolean;
    errors: PartyValidationError[];
  };
}

const isIntegerInRange = (value: unknown, minimum: number, maximum: number) =>
  typeof value === "number" && Number.isInteger(value) && value >= minimum && value <= maximum;

const isOneOf = <T extends string>(value: unknown, values: readonly T[]): value is T =>
  typeof value === "string" && values.includes(value as T);

function travelerVariantKey(characterId: string): string | null {
  if (characterId === "traveler" || characterId.startsWith("traveler-") || characterId.startsWith("traveler_")) {
    return "traveler";
  }
  return null;
}

export function getTravelerVariantKey(characterId: string): string | null {
  return travelerVariantKey(characterId);
}

function error(
  code: PartyValidationErrorCode,
  message: string,
  details: Pick<PartyValidationError, "field" | "slotIndex"> = {},
): PartyValidationError {
  return { code, message, ...details };
}

function characterLabel(character: Character | undefined, characterId: string) {
  return character?.name ?? characterId;
}

function emptyResult(saveErrors: PartyValidationError[], analyzeErrors: PartyValidationError[]): PartyValidationResult {
  return {
    canSave: saveErrors.length === 0,
    canAnalyze: analyzeErrors.length === 0,
    saveErrors,
    analyzeErrors,
    errors: [...saveErrors, ...analyzeErrors],
    save: { valid: saveErrors.length === 0, errors: saveErrors },
    analyze: { valid: analyzeErrors.length === 0, errors: analyzeErrors },
  };
}

/**
 * 下書きの保存可否と、分析投入可否を別々に判定する。
 *
 * 保存は編成名・4スロット構造・編集値の範囲・キャラクター重複だけを
 * 必須とし、未選択スロットを許容する。分析ではその保存条件に加えて
 * 全スロットのキャラクターと武器、およびカタログ上の武器種一致を要求する。
 */
export function validatePartyDraft(draft: PartyDraft, catalog?: Catalog): PartyValidationResult {
  const saveErrors: PartyValidationError[] = [];
  const analyzeErrors: PartyValidationError[] = [];

  if (typeof draft?.name !== "string" || draft.name.trim().length === 0) {
    saveErrors.push(error("name-required", "編成名を入力してください。", { field: "name" }));
  } else if (Array.from(draft.name).length > 40) {
    saveErrors.push(error("name-too-long", "編成名は40文字以内で入力してください。", { field: "name" }));
  }

  const members = draft?.members;
  if (!Array.isArray(members) || members.length !== PARTY_SLOT_COUNT) {
    const structureError = error("members-structure", "編成は4スロットで構成してください。", { field: "members" });
    saveErrors.push(structureError);
    analyzeErrors.push(structureError);
    return emptyResult(saveErrors, analyzeErrors);
  }

  const characterById = new Map((catalog?.characters ?? []).map((character) => [character.id, character]));
  const weaponById = new Map((catalog?.weapons ?? []).map((weapon) => [weapon.id, weapon]));
  const seenCharacters = new Map<string, number>();
  const seenTravelerVariants = new Map<string, number>();
  const baseAnalyzeErrors: PartyValidationError[] = [];

  members.forEach((member, position) => {
    const expectedSlotIndex = position as SlotIndex;
    const slotLabel = `スロット${position + 1}`;
    if (!member || member.slotIndex !== expectedSlotIndex) {
      const slotError = error("slot-index", `${slotLabel}のスロット番号が不正です。`, {
        field: `members.${position}.slotIndex`,
        slotIndex: position,
      });
      saveErrors.push(slotError);
      baseAnalyzeErrors.push(slotError);
      return;
    }

    if (!isIntegerInRange(member.constellation, 0, 6)) {
      saveErrors.push(
        error("constellation-range", `${slotLabel}の命ノ星座は0〜6で指定してください。`, {
          field: `members.${position}.constellation`,
          slotIndex: position,
        }),
      );
    }
    if (!isIntegerInRange(member.refinement, 1, 5)) {
      saveErrors.push(
        error("refinement-range", `${slotLabel}の精錬ランクは1〜5で指定してください。`, {
          field: `members.${position}.refinement`,
          slotIndex: position,
        }),
      );
    }
    if (!isOneOf<BuildIntent>(member.role, BUILD_INTENTS)) {
      saveErrors.push(
        error("role-invalid", `${slotLabel}の役割を選択してください。`, {
          field: `members.${position}.role`,
          slotIndex: position,
        }),
      );
    }
    if (!isOneOf<ReactionOwnership>(member.reactionOwnership, REACTION_OWNERSHIPS)) {
      saveErrors.push(
        error("reaction-ownership-invalid", `${slotLabel}の反応担当を選択してください。`, {
          field: `members.${position}.reactionOwnership`,
          slotIndex: position,
        }),
      );
    }
    if (!isOneOf<EnergyPriority>(member.energyPriority, ENERGY_PRIORITIES)) {
      saveErrors.push(
        error("energy-priority-invalid", `${slotLabel}の元素エネルギー方針を選択してください。`, {
          field: `members.${position}.energyPriority`,
          slotIndex: position,
        }),
      );
    }
    if (!isOneOf<SurvivabilityPriority>(member.survivabilityPriority, SURVIVABILITY_PRIORITIES)) {
      saveErrors.push(
        error("survivability-priority-invalid", `${slotLabel}の耐久方針を選択してください。`, {
          field: `members.${position}.survivabilityPriority`,
          slotIndex: position,
        }),
      );
    }

    if (typeof member.characterId === "string" && member.characterId.length > 0) {
      const previousPosition = seenCharacters.get(member.characterId);
      if (previousPosition !== undefined) {
        saveErrors.push(
          error("duplicate-character", `${slotLabel}とスロット${previousPosition + 1}で同じキャラクターが選択されています。`, {
            field: `members.${position}.characterId`,
            slotIndex: position,
          }),
        );
      } else {
        seenCharacters.set(member.characterId, position);
      }

      const travelerKey = travelerVariantKey(member.characterId);
      if (travelerKey !== null) {
        const previousTravelerPosition = seenTravelerVariants.get(travelerKey);
        if (previousTravelerPosition !== undefined && previousTravelerPosition !== position) {
          saveErrors.push(
            error(
              "traveler-variant-conflict",
              `旅人のvariantは同じ編成に複数入れられません（スロット${previousTravelerPosition + 1}と${position + 1}）。`,
              { field: `members.${position}.characterId`, slotIndex: position },
            ),
          );
        } else {
          seenTravelerVariants.set(travelerKey, position);
        }
      }
    }

    if (member.characterId === null || member.characterId === undefined || member.characterId === "") {
      baseAnalyzeErrors.push(
        error("character-required", `${slotLabel}のキャラクターを選択してください。`, {
          field: `members.${position}.characterId`,
          slotIndex: position,
        }),
      );
    }
    if (member.weaponId === null || member.weaponId === undefined || member.weaponId === "") {
      baseAnalyzeErrors.push(
        error("weapon-required", `${slotLabel}の武器を選択してください。`, {
          field: `members.${position}.weaponId`,
          slotIndex: position,
        }),
      );
    }
  });

  analyzeErrors.push(...saveErrors, ...baseAnalyzeErrors);

  if (catalog !== undefined) {
    if (catalog.characters.length === 0 || catalog.weapons.length === 0) {
      analyzeErrors.push(error("catalog-required", "分析に使うキャラクターと武器のカタログが必要です。"));
    }

    members.forEach((member, position) => {
      if (!member || member.slotIndex !== position) return;
      const slotLabel = `スロット${position + 1}`;
      const character = typeof member.characterId === "string" ? characterById.get(member.characterId) : undefined;
      const weapon = typeof member.weaponId === "string" ? weaponById.get(member.weaponId) : undefined;
      if (member.characterId && !character) {
        analyzeErrors.push(
          error("character-not-found", `${slotLabel}のキャラクター「${member.characterId}」がカタログにありません。`, {
            field: `members.${position}.characterId`,
            slotIndex: position,
          }),
        );
      }
      if (member.weaponId && !weapon) {
        analyzeErrors.push(
          error("weapon-not-found", `${slotLabel}の武器「${member.weaponId}」がカタログにありません。`, {
            field: `members.${position}.weaponId`,
            slotIndex: position,
          }),
        );
      }
      if (character && weapon && character.weaponType !== weapon.weaponType) {
        analyzeErrors.push(
          error(
            "weapon-type-mismatch",
            `${slotLabel}の武器種が一致していません（${characterLabel(character, character.id)}は${character.weaponType}）。`,
            { field: `members.${position}.weaponId`, slotIndex: position },
          ),
        );
      }
    });
  }

  return emptyResult(saveErrors, analyzeErrors);
}

export type { PartyDraft, PartyMemberDraft } from "./partyTypes";
