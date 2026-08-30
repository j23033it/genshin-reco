import { useMemo, useState } from "react";
import type { ChangeEvent } from "react";
import type { Catalog, Character, Weapon } from "../../domain/catalogTypes";
import { cn } from "../../lib/cn";
import {
  validatePartyDraft,
  type PartyValidationError,
} from "./partyDraft";
import {
  ENERGY_PRIORITY_OPTIONS,
  REACTION_OWNERSHIP_OPTIONS,
  ROLE_OPTIONS,
  SURVIVABILITY_PRIORITY_OPTIONS,
  type PartyDraft,
  type PartyMemberDraft,
  type SlotIndex,
} from "./partyTypes";

export interface PartyBuilderProps {
  catalog: Catalog;
  draft: PartyDraft;
  onChange: (draft: PartyDraft) => void;
  onSave: (draft: PartyDraft) => void;
  onAnalyze: (draft: PartyDraft) => void;
  disabled?: boolean;
}

const SLOT_INDICES: readonly SlotIndex[] = [0, 1, 2, 3];

function updateMember(draft: PartyDraft, slotIndex: SlotIndex, update: (member: PartyMemberDraft) => PartyMemberDraft) {
  const members = [...draft.members] as [PartyMemberDraft, PartyMemberDraft, PartyMemberDraft, PartyMemberDraft];
  members[slotIndex] = update(members[slotIndex]);
  return { ...draft, members };
}

function validationMessages(errors: PartyValidationError[]) {
  return errors.map(({ message, field }) => ({ message, field }));
}

function ErrorMessages({
  errors,
  id,
  title,
}: {
  errors: PartyValidationError[];
  id: string;
  title: string;
}) {
  if (errors.length === 0) return null;
  const messages = validationMessages(errors);
  return (
    <div id={id} className="mt-3 rounded-lg border border-amber-300/40 bg-amber-300/10 p-3 text-sm text-amber-100" role="alert">
      <p className="font-semibold">{title}</p>
      <ul className="mt-2 list-disc space-y-1 pl-5 text-pretty leading-6">
        {messages.map(({ message, field }, index) => (
          <li key={`${field ?? "error"}-${index}`}>{message}</li>
        ))}
      </ul>
    </div>
  );
}

function SelectField({
  id,
  label,
  value,
  onChange,
  options,
  disabled,
  describedBy,
}: {
  id: string;
  label: string;
  value: string | number;
  onChange: (event: ChangeEvent<HTMLSelectElement>) => void;
  options: ReadonlyArray<{ value: string | number; label: string }>;
  disabled: boolean;
  describedBy?: string;
}) {
  return (
    <div className="min-w-0">
      <label htmlFor={id} className="text-sm font-semibold text-slate-200">
        {label}
      </label>
      <select
        id={id}
        className="mt-2 min-h-11 w-full min-w-0 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
        value={value}
        onChange={onChange}
        disabled={disabled}
        aria-describedby={describedBy}
      >
        {options.map((option) => (
          <option key={String(option.value)} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function CharacterOption({ character, disabled }: { character: Character; disabled: boolean }) {
  return (
    <option value={character.id} disabled={disabled}>
      {character.name}（{character.element}・{character.weaponType}）
      {disabled ? " — 使用中" : ""}
    </option>
  );
}

function WeaponOption({ weapon, disabled }: { weapon: Weapon; disabled: boolean }) {
  return (
    <option value={weapon.id} disabled={disabled}>
      {weapon.name}（★{weapon.rarity}）{disabled ? " — 使用不可" : ""}
    </option>
  );
}

function PartySlot({
  catalog,
  characterById,
  weaponById,
  member,
  slotIndex,
  otherCharacterIds,
  otherTravelerVariants,
  onChange,
  disabled,
}: {
  catalog: Catalog;
  characterById: Map<string, Character>;
  weaponById: Map<string, Weapon>;
  member: PartyMemberDraft;
  slotIndex: SlotIndex;
  otherCharacterIds: Set<string>;
  otherTravelerVariants: Set<string>;
  onChange: (slotIndex: SlotIndex, update: (member: PartyMemberDraft) => PartyMemberDraft) => void;
  disabled: boolean;
}) {
  const [characterSearch, setCharacterSearch] = useState("");
  const character = member.characterId ? characterById.get(member.characterId) : undefined;
  const weapon = member.weaponId ? weaponById.get(member.weaponId) : undefined;
  const normalizedCharacterSearch = characterSearch.trim().toLocaleLowerCase("ja");
  const matchingCharacters = useMemo(
    () =>
      catalog.characters.filter(
        (candidate) =>
          normalizedCharacterSearch.length === 0 ||
          [candidate.name, candidate.element, candidate.weaponType].some((value) =>
            value.toLocaleLowerCase("ja").includes(normalizedCharacterSearch),
          ),
      ),
    [catalog.characters, normalizedCharacterSearch],
  );
  const characterOptions = useMemo(
    () => {
      if (!character || matchingCharacters.some((candidate) => candidate.id === character.id)) {
        return matchingCharacters;
      }
      return [character, ...matchingCharacters];
    },
    [character, matchingCharacters],
  );
  const weaponOptions = useMemo(
    () => (character ? catalog.weapons.filter((weapon) => weapon.weaponType === character.weaponType) : []),
    [catalog.weapons, character],
  );
  const currentWeaponIsVisible = member.weaponId !== null && weaponOptions.some((weapon) => weapon.id === member.weaponId);
  const selectedTravelerKey = member.characterId?.startsWith("traveler-") || member.characterId?.startsWith("traveler_") || member.characterId === "traveler" ? "traveler" : null;

  return (
    <article className="min-w-0 rounded-xl border border-slate-800 bg-slate-900/70 p-4" data-testid={`party-slot-${slotIndex + 1}`}>
      <h3 className="text-balance text-lg font-semibold text-slate-100">スロット{slotIndex + 1}</h3>

      <div className="mt-4 space-y-4">
        <div className="min-w-0">
          <label htmlFor={`party-character-search-${slotIndex}`} className="text-sm font-semibold text-slate-200">
            キャラクターを検索
          </label>
          <input
            id={`party-character-search-${slotIndex}`}
            type="search"
            value={characterSearch}
            onChange={(event) => setCharacterSearch(event.target.value)}
            disabled={disabled}
            className="mt-2 min-h-11 w-full min-w-0 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-500 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
            placeholder="名前・元素・武器種"
            aria-describedby={`party-character-search-result-${slotIndex}`}
          />
          <p
            id={`party-character-search-result-${slotIndex}`}
            className="mt-2 text-xs text-slate-400"
            role="status"
            aria-live="polite"
          >
            {normalizedCharacterSearch.length === 0
              ? `${catalog.characters.length}人から選択できます。`
              : `${matchingCharacters.length}人が一致しました。`}
          </p>
        </div>

        <div className="min-w-0">
          <label htmlFor={`party-character-${slotIndex}`} className="text-sm font-semibold text-slate-200">
            キャラクター
          </label>
          <select
            id={`party-character-${slotIndex}`}
            className="mt-2 min-h-11 w-full min-w-0 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
            value={member.characterId ?? ""}
            onChange={(event) => {
              const characterId = event.target.value || null;
              const nextCharacter = characterId ? characterById.get(characterId) : undefined;
              onChange(slotIndex, (previousMember) => {
                const previousCharacter = previousMember.characterId
                  ? characterById.get(previousMember.characterId)
                  : undefined;
                const sameWeaponType =
                  nextCharacter !== undefined &&
                  previousCharacter !== undefined &&
                  nextCharacter.weaponType === previousCharacter.weaponType;
                return {
                  ...previousMember,
                  characterId,
                  weaponId: sameWeaponType ? previousMember.weaponId : null,
                  refinement: sameWeaponType ? previousMember.refinement : 1,
                };
              });
            }}
            disabled={disabled}
          >
            <option value="">キャラクターを選択</option>
            {characterOptions.map((candidate) => {
              const travelerKey = candidate.id.startsWith("traveler-") || candidate.id.startsWith("traveler_") || candidate.id === "traveler" ? "traveler" : null;
              const optionDisabled =
                otherCharacterIds.has(candidate.id) ||
                (travelerKey !== null && otherTravelerVariants.has(travelerKey));
              return <CharacterOption key={candidate.id} character={candidate} disabled={optionDisabled} />;
            })}
            {matchingCharacters.length === 0 && <option disabled>一致するキャラクターはいません</option>}
          </select>
          {member.characterId && !character && (
            <p className="mt-2 text-xs leading-5 text-amber-200">このキャラクターは現在のカタログにありません。</p>
          )}
          {selectedTravelerKey !== null && otherTravelerVariants.has(selectedTravelerKey) && (
            <p className="mt-2 text-xs leading-5 text-amber-200">旅人のvariantが別スロットでも選択されています。</p>
          )}
        </div>

        <div className="min-w-0">
          <label htmlFor={`party-weapon-${slotIndex}`} className="text-sm font-semibold text-slate-200">
            武器
          </label>
          <select
            id={`party-weapon-${slotIndex}`}
            className="mt-2 min-h-11 w-full min-w-0 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
            value={member.weaponId ?? ""}
            onChange={(event) => {
              const weaponId = event.target.value || null;
              onChange(slotIndex, (previousMember) => ({ ...previousMember, weaponId }));
            }}
            disabled={disabled || character === undefined}
          >
            <option value="">{character ? "武器を選択" : "先にキャラクターを選択"}</option>
            {member.weaponId && !currentWeaponIsVisible && (
              <option value={member.weaponId} disabled>
                現在の武器（種別不一致）
              </option>
            )}
            {weaponOptions.map((weapon) => (
              <WeaponOption key={weapon.id} weapon={weapon} disabled={false} />
            ))}
          </select>
          {character && <p className="mt-2 text-xs text-slate-400">武器種: {character.weaponType}</p>}
        </div>

        <div className="grid min-w-0 grid-cols-2 gap-3">
          <SelectField
            id={`party-constellation-${slotIndex}`}
            label="命ノ星座"
            value={member.constellation}
            onChange={(event) => {
              const constellation = Number(event.target.value) as PartyMemberDraft["constellation"];
              onChange(slotIndex, (previousMember) => ({ ...previousMember, constellation }));
            }}
            disabled={disabled}
            options={[0, 1, 2, 3, 4, 5, 6].map((value) => ({ value, label: `C${value}` }))}
          />
          <SelectField
            id={`party-refinement-${slotIndex}`}
            label="精錬"
            value={member.refinement}
            onChange={(event) => {
              const refinement = Number(event.target.value) as PartyMemberDraft["refinement"];
              onChange(slotIndex, (previousMember) => ({ ...previousMember, refinement }));
            }}
            disabled={disabled}
            options={[1, 2, 3, 4, 5].map((value) => ({ value, label: `R${value}` }))}
          />
        </div>

        <fieldset className="min-w-0 space-y-3 border-0 p-0">
          <legend className="text-sm font-semibold text-slate-200">ビルド方針</legend>
          <SelectField
            id={`party-role-${slotIndex}`}
            label="役割"
            value={member.role}
            onChange={(event) => {
              const role = event.target.value as PartyMemberDraft["role"];
              onChange(slotIndex, (previousMember) => ({ ...previousMember, role }));
            }}
            disabled={disabled}
            options={ROLE_OPTIONS}
          />
          <SelectField
            id={`party-reaction-${slotIndex}`}
            label="反応担当"
            value={member.reactionOwnership}
            onChange={(event) => {
              const reactionOwnership = event.target.value as PartyMemberDraft["reactionOwnership"];
              onChange(slotIndex, (previousMember) => ({ ...previousMember, reactionOwnership }));
            }}
            disabled={disabled}
            options={REACTION_OWNERSHIP_OPTIONS}
          />
          <SelectField
            id={`party-energy-${slotIndex}`}
            label="元素エネルギー方針"
            value={member.energyPriority}
            onChange={(event) => {
              const energyPriority = event.target.value as PartyMemberDraft["energyPriority"];
              onChange(slotIndex, (previousMember) => ({ ...previousMember, energyPriority }));
            }}
            disabled={disabled}
            options={ENERGY_PRIORITY_OPTIONS}
          />
          <SelectField
            id={`party-survivability-${slotIndex}`}
            label="耐久方針"
            value={member.survivabilityPriority}
            onChange={(event) => {
              const survivabilityPriority = event.target.value as PartyMemberDraft["survivabilityPriority"];
              onChange(slotIndex, (previousMember) => ({ ...previousMember, survivabilityPriority }));
            }}
            disabled={disabled}
            options={SURVIVABILITY_PRIORITY_OPTIONS}
          />
        </fieldset>

        {(character?.imageUrl || weapon?.imageUrl) && (
          <div className="flex flex-wrap gap-2" aria-label="選択中のアイコン">
            {character?.imageUrl && (
              <img
                src={character.imageUrl}
                alt={`${character.name}のアイコン`}
                className="h-12 w-12 rounded-lg border border-slate-700 object-cover"
              />
            )}
            {weapon?.imageUrl && (
              <img
                src={weapon.imageUrl}
                alt={`${weapon.name}のアイコン`}
                className="h-12 w-12 rounded-lg border border-slate-700 object-cover"
              />
            )}
          </div>
        )}
      </div>
    </article>
  );
}

export function PartyBuilder({ catalog, draft, onChange, onSave, onAnalyze, disabled = false }: PartyBuilderProps) {
  const characterById = useMemo(
    () => new Map(catalog.characters.map((character) => [character.id, character])),
    [catalog.characters],
  );
  const weaponById = useMemo(
    () => new Map(catalog.weapons.map((weapon) => [weapon.id, weapon])),
    [catalog.weapons],
  );
  const selectedCharacterIds = useMemo(
    () => new Set(draft.members.map((member) => member.characterId).filter((characterId): characterId is string => characterId !== null && characterId !== "")),
    [draft.members],
  );
  const selectedTravelerVariants = useMemo(
    () =>
      new Set(
        draft.members
          .map((member) => member.characterId)
          .filter((characterId): characterId is string => characterId !== null && characterId !== "")
          .filter((characterId) => characterId === "traveler" || characterId.startsWith("traveler-") || characterId.startsWith("traveler_"))
          .map(() => "traveler"),
      ),
    [draft.members],
  );
  const validation = useMemo(() => validatePartyDraft(draft, catalog), [draft, catalog]);

  const handleMemberChange = (slotIndex: SlotIndex, update: (member: PartyMemberDraft) => PartyMemberDraft) => {
    onChange(updateMember(draft, slotIndex, update));
  };

  const handleNameChange = (event: ChangeEvent<HTMLInputElement>) => {
    onChange({ ...draft, name: event.target.value });
  };

  return (
    <section className="min-w-0 space-y-8 text-slate-100" data-testid="party-builder" aria-labelledby="party-builder-heading">
      <header className="space-y-3">
        <p className="text-sm font-semibold text-amber-400">編成エディター</p>
        <h2 id="party-builder-heading" className="text-balance text-3xl font-bold">4人編成を作成</h2>
        <p className="max-w-3xl text-pretty leading-7 text-slate-300">
          キャラクターと武器を選ぶと、同じ武器種の候補だけが表示されます。保存は途中の下書きでもできますが、分析には4人分の選択が必要です。
        </p>
      </header>

      <div className="max-w-xl">
        <label htmlFor="party-name" className="text-sm font-semibold text-slate-200">
          編成名
        </label>
        <input
          id="party-name"
          type="text"
          value={draft.name}
          onChange={handleNameChange}
          maxLength={40}
          disabled={disabled}
          aria-invalid={validation.saveErrors.some((validationError) => validationError.field === "name")}
          aria-describedby="party-name-help"
          className="mt-2 min-h-11 w-full rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-slate-100 placeholder:text-slate-500 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
          placeholder="例: 蒸発パーティー"
        />
        <p id="party-name-help" className="mt-2 text-sm text-slate-400">1〜40文字。途中の編成は名前を付けて保存できます。</p>
      </div>

      <div className="min-w-0 overflow-x-auto pb-3" aria-label="編成4スロット">
        <div className="grid min-w-[1120px] grid-cols-4 gap-4">
          {SLOT_INDICES.map((slotIndex) => {
            const member = draft.members[slotIndex];
            const otherCharacterIds = new Set(selectedCharacterIds);
            if (member.characterId) otherCharacterIds.delete(member.characterId);
            const otherTravelerVariants = new Set(selectedTravelerVariants);
            if (member.characterId && (member.characterId === "traveler" || member.characterId.startsWith("traveler-") || member.characterId.startsWith("traveler_"))) {
              otherTravelerVariants.delete("traveler");
            }
            return (
              <PartySlot
                key={slotIndex}
                catalog={catalog}
                characterById={characterById}
                weaponById={weaponById}
                member={member}
                slotIndex={slotIndex}
                otherCharacterIds={otherCharacterIds}
                otherTravelerVariants={otherTravelerVariants}
                onChange={handleMemberChange}
                disabled={disabled}
              />
            );
          })}
        </div>
      </div>

      <div className="grid gap-4 border-t border-slate-800 pt-6 sm:grid-cols-2">
        <div>
          <button
            type="button"
            className={cn(
              "inline-flex min-h-11 w-full items-center justify-center rounded-lg px-5 py-3 font-semibold shadow-sm",
              validation.canSave && !disabled
                ? "bg-amber-400 text-slate-950 hover:bg-amber-300"
                : "cursor-not-allowed bg-slate-700 text-slate-400",
            )}
            onClick={() => onSave(draft)}
            disabled={disabled || !validation.canSave}
            aria-describedby={validation.saveErrors.length > 0 ? "party-save-errors" : undefined}
          >
            保存
          </button>
          <ErrorMessages errors={validation.saveErrors} id="party-save-errors" title="保存できません" />
        </div>
        <div>
          <button
            type="button"
            className={cn(
              "inline-flex min-h-11 w-full items-center justify-center rounded-lg px-5 py-3 font-semibold shadow-sm",
              validation.canAnalyze && !disabled
                ? "bg-amber-400 text-slate-950 hover:bg-amber-300"
                : "cursor-not-allowed bg-slate-700 text-slate-400",
            )}
            onClick={() => onAnalyze(draft)}
            disabled={disabled || !validation.canAnalyze}
            aria-describedby={validation.analyzeErrors.length > 0 ? "party-analyze-errors" : undefined}
          >
            分析を開始
          </button>
          <ErrorMessages errors={validation.analyzeErrors} id="party-analyze-errors" title="分析できません" />
        </div>
      </div>

      <p className="sr-only" role="status" aria-live="polite">
        {validation.canAnalyze ? "分析を開始できます。" : "分析には4人分のキャラクターと武器が必要です。"}
      </p>
    </section>
  );
}

export default PartyBuilder;

export type { PartyDraft, PartyMemberDraft } from "./partyTypes";
