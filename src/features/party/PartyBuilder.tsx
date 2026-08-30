import { useMemo, useState } from "react";
import type { ChangeEvent } from "react";
import type { AnalysisMode } from "../../domain/analysisTypes";
import type { Catalog, Character, Weapon } from "../../domain/catalogTypes";
import { cn } from "../../lib/cn";
import {
  validatePartyDraft,
  type PartyValidationError,
} from "./partyDraft";
import {
  type PartyDraft,
  type PartyMemberDraft,
  type SlotIndex,
} from "./partyTypes";

export interface PartyBuilderProps {
  catalog: Catalog;
  draft: PartyDraft;
  onChange: (draft: PartyDraft) => void;
  onAnalyze: (draft: PartyDraft) => void;
  analysisMode?: AnalysisMode;
  onAnalysisModeChange?: (mode: AnalysisMode) => void;
  mode?: "create" | "edit";
  actionError?: string | null;
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

export function PartyBuilder({
  catalog,
  draft,
  onChange,
  onAnalyze,
  analysisMode,
  onAnalysisModeChange,
  mode = "create",
  actionError = null,
  disabled = false,
}: PartyBuilderProps) {
  const draftIdentity = draft.partyId ?? draft.id ?? "new-party";
  const [uncontrolledAnalysisMode, setUncontrolledAnalysisMode] = useState<AnalysisMode>("normal");
  const selectedAnalysisMode = analysisMode ?? uncontrolledAnalysisMode;
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

  const handleAnalysisModeChange = (nextMode: AnalysisMode) => {
    setUncontrolledAnalysisMode(nextMode);
    onAnalysisModeChange?.(nextMode);
  };

  return (
    <section className="min-w-0 space-y-8 text-slate-100" data-testid="party-builder" aria-labelledby="party-builder-heading">
      <header className="space-y-3">
        <p className="text-sm font-semibold text-amber-400">{mode === "edit" ? "保存編成" : "新規編成"}</p>
        <h2 id="party-builder-heading" className="text-balance text-3xl font-bold">
          {mode === "edit" ? "保存編成を編集" : "4人編成を作成"}
        </h2>
        <p className="max-w-3xl text-pretty leading-7 text-slate-300">
          キャラクターと武器を選ぶと、同じ武器種の候補だけが表示されます。役割や反応担当などのビルド方針は、編成と検証済みの根拠から分析時に判断します。
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
          aria-invalid={validation.analyzeErrors.some((validationError) => validationError.field === "name")}
          aria-describedby="party-name-help"
          className="mt-2 min-h-11 w-full rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-slate-100 placeholder:text-slate-500 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60"
          placeholder="例: 蒸発パーティー"
        />
        <p id="party-name-help" className="mt-2 text-pretty text-sm text-slate-400">
          1〜40文字。{mode === "edit" ? "変更内容は分析更新時に保存されます。" : "分析が完了すると保存編成へ追加されます。"}
        </p>
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
                key={`${draftIdentity}-${slotIndex}`}
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

      <div className="max-w-xl border-t border-slate-800 pt-6">
        <div>
          <fieldset
            className="mb-4 rounded-lg border border-slate-800 bg-slate-900/40 p-4"
            disabled={disabled}
            aria-describedby={`analysis-mode-help-${draftIdentity}`}
          >
            <legend className="px-1 text-sm font-semibold text-slate-200">分析モード</legend>
            <div className="grid gap-3 sm:grid-cols-2">
              <label
                htmlFor={`analysis-mode-normal-${draftIdentity}`}
                className={cn(
                  "flex min-h-11 min-w-0 cursor-pointer items-center gap-3 rounded-lg border px-3 py-2 focus-within:outline focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-amber-400",
                  selectedAnalysisMode === "normal"
                    ? "border-amber-400/70 bg-amber-400/10"
                    : "border-slate-700 hover:border-slate-500",
                  disabled && "cursor-not-allowed opacity-60",
                )}
              >
                <input
                  id={`analysis-mode-normal-${draftIdentity}`}
                  type="radio"
                  name={`analysis-mode-${draftIdentity}`}
                  value="normal"
                  checked={selectedAnalysisMode === "normal"}
                  onChange={() => handleAnalysisModeChange("normal")}
                  disabled={disabled}
                  aria-label="通常"
                  aria-describedby={`analysis-mode-normal-help-${draftIdentity}`}
                  className="size-4 accent-amber-400"
                />
                <span className="min-w-0">
                  <span className="block font-semibold text-slate-100">通常</span>
                  <span
                    id={`analysis-mode-normal-help-${draftIdentity}`}
                    className="mt-1 block text-xs text-slate-400"
                  >
                    精度優先
                  </span>
                </span>
              </label>
              <label
                htmlFor={`analysis-mode-fast-${draftIdentity}`}
                className={cn(
                  "flex min-h-11 min-w-0 cursor-pointer items-center gap-3 rounded-lg border px-3 py-2 focus-within:outline focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-amber-400",
                  selectedAnalysisMode === "fast"
                    ? "border-amber-400/70 bg-amber-400/10"
                    : "border-slate-700 hover:border-slate-500",
                  disabled && "cursor-not-allowed opacity-60",
                )}
              >
                <input
                  id={`analysis-mode-fast-${draftIdentity}`}
                  type="radio"
                  name={`analysis-mode-${draftIdentity}`}
                  value="fast"
                  checked={selectedAnalysisMode === "fast"}
                  onChange={() => handleAnalysisModeChange("fast")}
                  disabled={disabled}
                  aria-label="高速"
                  aria-describedby={`analysis-mode-fast-help-${draftIdentity}`}
                  className="size-4 accent-amber-400"
                />
                <span className="min-w-0">
                  <span className="block font-semibold text-slate-100">高速</span>
                  <span
                    id={`analysis-mode-fast-help-${draftIdentity}`}
                    className="mt-1 block text-xs text-slate-400"
                  >
                    待ち時間優先
                  </span>
                </span>
              </label>
            </div>
            <p id={`analysis-mode-help-${draftIdentity}`} className="mt-3 text-xs leading-5 text-slate-400">
              分析モードは保存編成には記録されません。
            </p>
          </fieldset>
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
            {mode === "edit" ? "分析を更新" : "分析を開始"}
          </button>
          <ErrorMessages errors={validation.analyzeErrors} id="party-analyze-errors" title="分析できません" />
          {actionError ? (
            <p className="mt-3 border-l-2 border-rose-400 pl-3 text-pretty text-sm leading-6 text-rose-200" role="alert">
              {actionError}
            </p>
          ) : null}
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
