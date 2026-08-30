import { useMemo } from "react";
import type { ArtifactSet, Catalog, Character, Weapon } from "../../domain/catalogTypes";
import type {
  ArtifactHalf,
  ArtifactPlan,
  BuildCondition,
  BuildVariant,
  CharacterBuildResolution,
  ResultValidity,
  StatPriority,
  TargetStatRange,
  TeamBuildResolution,
} from "../../domain/analysisTypes";
import type { PartyDraft, PartyMemberDraft } from "../party";

export interface TeamResultPanelProps {
  catalog: Catalog;
  party: PartyDraft | null;
  resolution: TeamBuildResolution | null;
  validity: ResultValidity;
  onChooseVariant?: (characterId: string, variantId: string) => void;
  choosingVariantKey?: string | null;
}

const RESOLUTION_STATUS_LABELS: Record<TeamBuildResolution["status"], string> = {
  resolved: "分析結果：確定",
  needs_user_choice: "分析結果：候補を選択してください",
  unresolved: "分析結果：解決できませんでした",
};

const OPERATOR_LABELS: Record<BuildCondition["operator"], string> = {
  equals: "一致",
  not_equals: "不一致",
  includes: "含む",
  gte: "以上",
  lte: "以下",
};

const TARGET_SCOPE_LABELS: Record<TargetStatRange["scope"], string> = {
  character_sheet_unbuffed: "戦闘前",
  character_sheet_with_static_team_effects: "固定チーム効果込み",
  in_combat_conditional: "戦闘中・条件付き",
};

const STAT_LABELS: Record<string, string> = {
  atk: "攻撃力",
  attack: "攻撃力",
  atk_percent: "攻撃力%",
  attack_percent: "攻撃力%",
  hp: "HP",
  hp_percent: "HP%",
  def: "防御力",
  defense: "防御力",
  def_percent: "防御力%",
  defense_percent: "防御力%",
  er: "元素チャージ効率",
  energy_recharge: "元素チャージ効率",
  em: "元素熟知",
  elemental_mastery: "元素熟知",
  crit_rate: "会心率",
  critical_rate: "会心率",
  crit_damage: "会心ダメージ",
  critical_damage: "会心ダメージ",
  healing_bonus: "与える治療効果",
  pyro_damage_bonus: "炎元素ダメージ",
  hydro_damage_bonus: "水元素ダメージ",
  electro_damage_bonus: "雷元素ダメージ",
  cryo_damage_bonus: "氷元素ダメージ",
  anemo_damage_bonus: "風元素ダメージ",
  geo_damage_bonus: "岩元素ダメージ",
  dendro_damage_bonus: "草元素ダメージ",
  physical_damage_bonus: "物理ダメージ",
};

function localizeStatLabel(stat: string) {
  const normalized = stat.trim().toLocaleLowerCase("en").replace(/[\s-]+/g, "_").replace(/%$/, "_percent");
  return STAT_LABELS[normalized] ?? (looksLikeRawIdentifier(stat) ? "未対応ステータス" : stat);
}

function looksLikeRawIdentifier(value: string) {
  const normalized = value.trim();
  return normalized.length > 0 && /^[a-z0-9_-]+$/i.test(normalized) && /[_\d-]/.test(normalized);
}

function displayTextOrFallback(value: string, fallback: string) {
  const normalized = value.trim();
  return !normalized || looksLikeRawIdentifier(normalized) ? fallback : normalized;
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("ja-JP", { maximumFractionDigits: 2 }).format(value);
}

function formatTargetRange(target: TargetStatRange) {
  const minimum = target.minimum === null ? null : `${formatNumber(target.minimum)}${target.unit === "percent" ? "%" : ""}`;
  const maximum = target.maximum === null ? null : `${formatNumber(target.maximum)}${target.unit === "percent" ? "%" : ""}`;
  if (minimum && maximum) return minimum === maximum ? minimum : `${minimum}～${maximum}`;
  if (minimum) return `${minimum}以上`;
  if (maximum) return `${maximum}以下`;
  return "指定なし";
}

function formatIncludedBonus(target: TargetStatRange) {
  return target.includedBonuses
    .map((bonus) => {
      const source = displayTextOrFallback(bonus.source, "編成効果");
      const condition = bonus.condition ? displayTextOrFallback(bonus.condition, "適用条件あり") : null;
      return `${source} +${formatNumber(bonus.amount)}%${condition ? `（${condition}）` : ""}`;
    })
    .join(" ／ ");
}

function formatConditionValue(value: BuildCondition["value"]) {
  if (typeof value === "boolean") return value ? "はい" : "いいえ";
  if (typeof value === "string") return displayTextOrFallback(value, "指定値");
  return String(value);
}

function formatCondition(condition: BuildCondition) {
  return condition.description || `${OPERATOR_LABELS[condition.operator]} ${formatConditionValue(condition.value)}`;
}

function findArtifactSet(catalog: Catalog, setId: string) {
  return catalog.artifactSets.find((artifactSet) => artifactSet.id === setId);
}

function effectGroupSets(catalog: Catalog, effectGroupId: string) {
  return catalog.artifactSets.filter((artifactSet) => artifactSet.twoPieceEffectGroupId === effectGroupId);
}

function artifactHalfLabel(half: ArtifactHalf, catalog: Catalog) {
  if (half.kind === "exact_set") return findArtifactSet(catalog, half.setId)?.name ?? "聖遺物セット";
  const names = effectGroupSets(catalog, half.effectGroupId).map((artifactSet) => artifactSet.name);
  if (names.length === 0) return "同一効果の聖遺物セット";
  const visibleNames = names.slice(0, 2).join("・");
  const remaining = names.length - 2;
  return `同一効果：${visibleNames}${remaining > 0 ? ` ほか${remaining}種` : ""}`;
}

function artifactPlanLabel(plan: ArtifactPlan, catalog: Catalog) {
  if (plan.type === "four_piece") return findArtifactSet(catalog, plan.setId)?.name ?? "聖遺物セット";
  return `${artifactHalfLabel(plan.first, catalog)} ＋ ${artifactHalfLabel(plan.second, catalog)}`;
}

function representativeArtifactSet(plan: ArtifactPlan, catalog: Catalog): ArtifactSet | undefined {
  if (plan.type === "four_piece") return findArtifactSet(catalog, plan.setId);
  if (plan.first.kind === "exact_set") return findArtifactSet(catalog, plan.first.setId);
  if (plan.second.kind === "exact_set") return findArtifactSet(catalog, plan.second.setId);
  return effectGroupSets(catalog, plan.first.effectGroupId)[0] ?? effectGroupSets(catalog, plan.second.effectGroupId)[0];
}

function ArtifactSummary({ plan, catalog }: { plan: ArtifactPlan; catalog: Catalog }) {
  const artifactSet = representativeArtifactSet(plan, catalog);
  const imageUrl = artifactSet?.pieceImageUrls.flower;
  const label = artifactPlanLabel(plan, catalog);
  return (
    <div className="flex min-w-0 items-center gap-3">
      {imageUrl ? <img src={imageUrl} alt={`${label}の聖遺物画像`} className="size-12 shrink-0 rounded-lg border border-slate-700 object-cover" /> : null}
      <p className="min-w-0 break-words text-pretty text-sm font-semibold text-slate-100">{label}</p>
    </div>
  );
}

function MainStatSummary({ variant }: { variant: BuildVariant }) {
  return (
    <p className="min-w-0 break-words text-pretty text-sm text-slate-300">
      砂：{localizeStatLabel(variant.mainStatPackage.sands)} ／ 杯：{localizeStatLabel(variant.mainStatPackage.goblet)} ／ 冠：{localizeStatLabel(variant.mainStatPackage.circlet)}
    </p>
  );
}

function SubstatSummary({ priorities }: { priorities: StatPriority[] }) {
  const sorted = [...priorities].sort((left, right) => left.rank - right.rank);
  return (
    <p className="min-w-0 break-words text-pretty text-sm text-slate-300">
      優先サブ：{sorted.length > 0 ? sorted.map((priority) => localizeStatLabel(priority.stat)).join(" > ") : "指定なし"}
    </p>
  );
}

function TargetSummary({ targets }: { targets: TargetStatRange[] }) {
  const calculatedTargets = targets.filter((target) => target.minimum !== null || target.maximum !== null);
  return (
    <div className="min-w-0">
      <p className="text-sm font-semibold text-slate-200">目標ステータス</p>
      {calculatedTargets.length > 0 ? (
        <ul className="mt-1 space-y-1 text-pretty text-sm text-slate-300">
          {calculatedTargets.map((target) => (
            <li key={`${target.stat}-${target.minimum}-${target.maximum}`} className="break-words">
              {localizeStatLabel(target.stat)}：<span className="tabular-nums">{formatTargetRange(target)}</span>
              <span className="ml-2 text-xs text-slate-400">（{TARGET_SCOPE_LABELS[target.scope]}）</span>
            </li>
          ))}
          {calculatedTargets.length < targets.length ? (
            <li className="text-amber-200">一部未算出（再分析が必要です）</li>
          ) : null}
        </ul>
      ) : (
        <p className="mt-1 text-pretty text-sm text-amber-200">未算出（再分析が必要です）</p>
      )}
    </div>
  );
}

function VariantSummary({ variant, catalog }: { variant: BuildVariant; catalog: Catalog }) {
  return (
    <div className="min-w-0 space-y-2">
      <ArtifactSummary plan={variant.artifactPlan} catalog={catalog} />
      <MainStatSummary variant={variant} />
    </div>
  );
}

function ChoiceCandidate({
  characterId,
  variant,
  candidateNumber,
  catalog,
  onChooseVariant,
  choosingVariantKey,
}: {
  characterId: string;
  variant: BuildVariant;
  candidateNumber: number;
  catalog: Catalog;
  onChooseVariant?: (characterId: string, variantId: string) => void;
  choosingVariantKey?: string | null;
}) {
  const selectionKey = `${characterId}:${variant.id}`;
  const isChoosing = choosingVariantKey === selectionKey;
  const choicePending = choosingVariantKey !== null && choosingVariantKey !== undefined;
  return (
    <li className="min-w-0 rounded-lg border border-amber-300/50 bg-amber-400/5 p-3">
      <button
        type="button"
        className="min-h-11 w-full rounded-md border border-amber-300/60 px-3 py-2 text-left text-amber-100 hover:bg-amber-300/10 focus-visible:outline-2 focus-visible:outline-amber-400 disabled:cursor-not-allowed disabled:opacity-60"
        onClick={() => onChooseVariant?.(characterId, variant.id)}
        disabled={!onChooseVariant || choicePending}
        aria-label={isChoosing ? `候補${candidateNumber}を保存中` : `候補${candidateNumber}を選択`}
        aria-busy={isChoosing || undefined}
        data-testid="variant-choice"
        data-character-id={characterId}
        data-variant-id={variant.id}
      >
        <span className="mb-2 block font-semibold">候補{candidateNumber}</span>
        <VariantSummary variant={variant} catalog={catalog} />
      </button>
    </li>
  );
}

function AlternativesDetails({
  member,
  resolutionStatus,
  catalog,
  onChooseVariant,
  choosingVariantKey,
}: {
  member: CharacterBuildResolution;
  resolutionStatus: TeamBuildResolution["status"];
  catalog: Catalog;
  onChooseVariant?: (characterId: string, variantId: string) => void;
  choosingVariantKey?: string | null;
}) {
  const isChoicePending = resolutionStatus === "needs_user_choice" && member.selectedVariantId === null;
  const alternatives = member.alternatives
    .map((variant, index) => ({ variant, candidateNumber: index + 1 }))
    .filter(({ variant }) => isChoicePending || variant.id !== member.selectedVariantId);

  if (alternatives.length === 0) return null;

  if (isChoicePending) {
    return (
      <section className="mt-4 space-y-2" aria-labelledby={`alternatives-${member.characterId}`}>
        <h4 id={`alternatives-${member.characterId}`} className="text-balance text-sm font-semibold text-slate-200">候補を選択</h4>
        <ul className="space-y-3">
          {alternatives.map(({ variant, candidateNumber }) => (
            <ChoiceCandidate
              key={variant.id}
              characterId={member.characterId}
              variant={variant}
              candidateNumber={candidateNumber}
              catalog={catalog}
              onChooseVariant={onChooseVariant}
              choosingVariantKey={choosingVariantKey}
            />
          ))}
        </ul>
      </section>
    );
  }

  return (
    <details className="mt-4 rounded-lg border border-slate-700 bg-slate-950/30 p-3">
      <summary className="cursor-pointer text-balance text-sm font-semibold text-slate-200">別候補（{alternatives.length}件）</summary>
      <ul className="mt-3 space-y-3">
        {alternatives.map(({ variant, candidateNumber }) => (
          <li key={variant.id} className="min-w-0 border-t border-slate-700 pt-3 first:border-t-0 first:pt-0">
            <p className="mb-2 text-sm font-semibold text-slate-300">候補{candidateNumber}</p>
            <VariantSummary variant={variant} catalog={catalog} />
          </li>
        ))}
      </ul>
    </details>
  );
}

function CharacterIdentity({
  character,
  weapon,
  partyMember,
}: {
  character: Character | undefined;
  weapon: Weapon | undefined;
  partyMember: PartyMemberDraft | undefined;
}) {
  const characterName = character?.name ?? "キャラクター未登録";
  const weaponName = weapon?.name ?? "武器未登録";
  return (
    <div className="space-y-3">
      <div className="flex min-w-0 items-center gap-3">
        {character?.imageUrl ? <img src={character.imageUrl} alt={`${character.name}のキャラクター画像`} className="size-14 shrink-0 rounded-lg border border-slate-700 object-cover" /> : null}
        <div className="min-w-0">
          <h3 className="break-words text-balance text-lg font-bold text-slate-100">{characterName}</h3>
          <p className="mt-1 tabular-nums text-sm text-slate-300">C{partyMember?.constellation ?? "—"}</p>
        </div>
      </div>
      <div className="flex min-w-0 items-center gap-3">
        {weapon?.imageUrl ? <img src={weapon.imageUrl} alt={`${weapon.name}の武器画像`} className="size-10 shrink-0 rounded-lg border border-slate-700 object-cover" /> : null}
        <p className="min-w-0 break-words text-pretty text-sm text-slate-200">
          {weaponName} <span className="tabular-nums text-slate-400">R{partyMember?.refinement ?? "—"}</span>
        </p>
      </div>
    </div>
  );
}

function CharacterResultCard({
  member,
  index,
  partyMember,
  catalog,
  resolutionStatus,
  onChooseVariant,
  choosingVariantKey,
}: {
  member: CharacterBuildResolution | null;
  index: number;
  partyMember: PartyMemberDraft | undefined;
  catalog: Catalog;
  resolutionStatus: TeamBuildResolution["status"];
  onChooseVariant?: (characterId: string, variantId: string) => void;
  choosingVariantKey?: string | null;
}) {
  const character = member ? catalog.characters.find((candidate) => candidate.id === member.characterId) : undefined;
  const weapon = partyMember?.weaponId ? catalog.weapons.find((candidate) => candidate.id === partyMember.weaponId) : undefined;

  if (!member) {
    return (
      <article className="min-w-0 rounded-xl border border-slate-700 bg-slate-900/60 p-4" aria-label={`キャラクター${index + 1}`} data-testid="character-result-card">
        <h3 className="text-balance text-base font-semibold text-slate-200">キャラクター{index + 1}</h3>
        <p className="mt-3 text-pretty text-sm leading-6 text-slate-400">この枠の分析結果はありません。</p>
      </article>
    );
  }

  const selectedVariant = member.selectedVariantId
    ? member.alternatives.find((variant) => variant.id === member.selectedVariantId) ?? null
    : null;
  const displayedVariant = selectedVariant ?? member.alternatives[0] ?? null;

  return (
    <article className="min-w-0 rounded-xl border border-slate-700 bg-slate-900/80 p-4" aria-label={`${character?.name ?? "キャラクター未登録"}の分析結果`} data-testid="character-result-card">
      <CharacterIdentity character={character} weapon={weapon} partyMember={partyMember} />
      {member.selectedVariantId ? (
        <p className="mt-3 inline-flex rounded-full border border-emerald-300/50 px-2 py-1 text-xs font-semibold text-emerald-200">
          選択済み
        </p>
      ) : null}
      {displayedVariant ? (
        <div className="mt-4 space-y-3 border-t border-slate-700 pt-4">
          <VariantSummary variant={displayedVariant} catalog={catalog} />
          <SubstatSummary priorities={displayedVariant.mainStatPackage.substatPriority} />
          <TargetSummary targets={displayedVariant.mainStatPackage.targetStats} />
        </div>
      ) : (
        <p className="mt-4 text-pretty text-sm leading-6 text-slate-400">表示できるビルド候補はありません。</p>
      )}
      <AlternativesDetails
        member={member}
        resolutionStatus={resolutionStatus}
        catalog={catalog}
        onChooseVariant={onChooseVariant}
        choosingVariantKey={choosingVariantKey}
      />
    </article>
  );
}

function EmptyResultState({ party }: { party: PartyDraft | null }) {
  const partyName = party?.name.trim();
  return (
    <div className="rounded-xl border border-dashed border-slate-600 bg-slate-900/60 p-6" data-testid="team-result-empty">
      <h3 className="text-balance text-lg font-semibold text-slate-100">分析結果はまだありません</h3>
      <p className="mt-2 break-words text-pretty text-sm leading-6 text-slate-300">
        {party
          ? `${partyName || "この編成"}はまだ分析されていません。編成を分析すると、キャラクターごとのビルド候補を表示できます。`
          : "保存済み編成を選択するか、新しい編成を作成して分析すると結果を表示できます。"}
      </p>
    </div>
  );
}

export function TeamResultPanel({ catalog, party, resolution, onChooseVariant, choosingVariantKey = null }: TeamResultPanelProps) {
  const partyMembers = useMemo(() => party?.members ?? [], [party]);
  const members = resolution ? Array.from({ length: 4 }, (_, index) => resolution.members[index] ?? null) : [];

  return (
    <section className="min-w-0 space-y-4" aria-labelledby="team-result-heading" data-testid="team-result-panel">
      <div>
        <h2 id="team-result-heading" className="text-balance text-xl font-bold text-slate-100">チーム分析結果</h2>
        {party?.name ? <p className="mt-1 break-words text-pretty font-semibold text-amber-300">{party.name}</p> : null}
        <p className="mt-1 text-pretty text-sm leading-6 text-slate-300">4人分のキャラクター、武器、聖遺物、ステータス目標を確認できます。</p>
      </div>

      {resolution === null ? (
        <EmptyResultState party={party} />
      ) : (
        <>
          <div className="rounded-lg border border-slate-700 bg-slate-900/70 p-4" role="status" aria-live="polite">
            <p className="text-pretty font-semibold text-slate-100">{RESOLUTION_STATUS_LABELS[resolution.status]}</p>
            {resolution.status === "needs_user_choice" ? <p className="mt-1 text-pretty text-sm leading-6 text-amber-200">未選択の候補ボタンから、各キャラクターの案を選んでください。</p> : null}
            {resolution.status === "unresolved" ? <p className="mt-1 text-pretty text-sm leading-6 text-rose-200">条件を満たす組み合わせを確定できませんでした。補足事項を確認してください。</p> : null}
          </div>

          <div className="grid min-w-0 grid-cols-1 gap-4 xl:grid-cols-2" aria-label="4人分の分析結果">
            {members.map((member, index) => (
              <CharacterResultCard
                key={member?.characterId ?? `empty-${index}`}
                member={member}
                index={index}
                partyMember={
                  member
                    ? partyMembers.find((partyMember) => partyMember.characterId === member.characterId)
                    : partyMembers[index]
                }
                catalog={catalog}
                resolutionStatus={resolution.status}
                onChooseVariant={onChooseVariant}
                choosingVariantKey={choosingVariantKey}
              />
            ))}
          </div>
        </>
      )}
    </section>
  );
}

export interface AnalysisNotesPanelProps {
  catalog: Catalog;
  resolution: TeamBuildResolution | null;
  validity: ResultValidity;
}

const VALIDITY_NOTES: Partial<Record<ResultValidity, string>> = {
  soft_stale: "以前の分析結果です。最新の入力と一致しない可能性があるため、利用前に再確認してください。",
  hard_stale: "古い分析結果です。利用する前に再分析してください。",
  invalid: "この分析結果は無効です。結果を利用せず、再分析してください。",
};

function selectedOrCandidate(member: CharacterBuildResolution) {
  return member.selectedVariantId
    ? member.alternatives.find((variant) => variant.id === member.selectedVariantId) ?? member.alternatives[0]
    : member.alternatives[0];
}

function localizeWarning(warning: string, catalog: Catalog) {
  const separatorIndex = warning.indexOf(": ");
  if (separatorIndex < 0) return warning;
  const characterId = warning.slice(0, separatorIndex);
  const character = catalog.characters.find((candidate) => candidate.id === characterId);
  if (character) return `${character.name}：${warning.slice(separatorIndex + 2)}`;
  return looksLikeRawIdentifier(characterId) ? `キャラクター：${warning.slice(separatorIndex + 2)}` : warning;
}

export function AnalysisNotesPanel({ catalog, resolution, validity }: AnalysisNotesPanelProps) {
  const notes = useMemo(() => {
    const values: string[] = [];
    const add = (value: string) => {
      const normalized = value.trim();
      if (normalized && !values.includes(normalized)) values.push(normalized);
    };

    resolution?.warnings.forEach((warning) => add(localizeWarning(warning, catalog)));
    resolution?.members.forEach((member) => {
      const variant = selectedOrCandidate(member);
      variant?.conditions.forEach((condition) => add(`適用条件：${formatCondition(condition)}`));
      variant?.mainStatPackage.conditions.forEach((condition) => add(`メインステータス条件：${formatCondition(condition)}`));
      variant?.mainStatPackage.targetStats.forEach((target) => {
        if (target.note) add(`目標値の注記（${localizeStatLabel(target.stat)}）：${target.note}`);
        if (target.includedBonuses.length > 0) {
          add(`目標値に含める効果（${localizeStatLabel(target.stat)}）：${formatIncludedBonus(target)}`);
        }
      });
    });
    const validityNote = VALIDITY_NOTES[validity];
    if (validityNote) add(validityNote);
    return values;
  }, [catalog, resolution, validity]);

  return (
    <section className="min-w-0 space-y-3" aria-labelledby="analysis-notes-heading" data-testid="analysis-notes-panel">
      <h2 id="analysis-notes-heading" className="text-balance text-lg font-semibold text-slate-100">補足事項</h2>
      {notes.length > 0 ? (
        <ul className="space-y-2 text-pretty text-sm leading-6 text-slate-300">
          {notes.map((note) => <li key={note} className="break-words border-l-2 border-amber-400 pl-3">{note}</li>)}
        </ul>
      ) : (
        <p className="text-pretty text-sm leading-6 text-slate-400">補足事項はありません</p>
      )}
    </section>
  );
}
