import { cn } from "../../lib/cn";
import type {
  ArtifactHalf,
  ArtifactPlan,
  BuildCondition,
  BuildVariant,
  CharacterBuildResolution,
  EvidenceClaim,
  EvidenceGrade,
  EvidenceVerification,
  MainStatPackage,
  ResultValidity,
  StatPriority,
  TargetScope,
  TargetStatRange,
  TeamBuildResolution,
} from "../../domain/analysisTypes";

export interface TeamResultPanelProps {
  resolution: TeamBuildResolution | null;
  validity: ResultValidity;
  onChooseVariant?: (characterId: string, variantId: string) => void;
}

const RESOLUTION_STATUS_LABELS: Record<TeamBuildResolution["status"], string> = {
  resolved: "分析結果：確定",
  needs_user_choice: "分析結果：候補を選択してください",
  unresolved: "分析結果：解決できませんでした",
};

const VALIDITY_LABELS: Record<ResultValidity, string> = {
  current: "最新の結果",
  soft_stale: "以前の結果（要再確認）",
  hard_stale: "古い結果（再分析が必要）",
  invalid: "無効な結果",
};

const TARGET_SCOPE_LABELS: Record<TargetScope, string> = {
  character_sheet_unbuffed: "キャラクター画面（無バフ）",
  character_sheet_with_static_team_effects: "キャラクター画面（固定チーム効果あり）",
  in_combat_conditional: "戦闘中（条件付き）",
};

const EVIDENCE_VERIFICATION_LABELS: Record<EvidenceVerification, string> = {
  host_exact_match: "同一ホストで完全一致",
  host_fuzzy_match: "同一ホストで候補一致",
  url_event_only: "URLイベントのみ確認",
  unverified: "未検証",
};

const EVIDENCE_CLAIM_LABELS: Record<EvidenceClaim["claimType"], string> = {
  artifact_plan: "聖遺物セット",
  main_stat_package: "メインステータス",
  substat_priority: "サブステータス優先度",
  target_stat: "目標ステータス",
  role: "役割",
  team_interaction: "チーム連携",
};

const EVIDENCE_GRADE_LABELS: Record<EvidenceGrade, string> = {
  A: "強い根拠",
  B: "標準的な根拠",
  C: "補助的な根拠",
};

function formatArtifactHalf(half: ArtifactHalf) {
  return half.kind === "exact_set" ? `セット：${half.setId}` : `効果グループ：${half.effectGroupId}`;
}

function formatArtifactPlan(plan: ArtifactPlan) {
  return plan.type === "four_piece"
    ? `4セット：${plan.setId}`
    : `2セット＋2セット：${formatArtifactHalf(plan.first)} ／ ${formatArtifactHalf(plan.second)}`;
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("ja-JP", { maximumFractionDigits: 2 }).format(value);
}

function formatTargetValue(value: number | null, unit: TargetStatRange["unit"]) {
  if (value === null) return null;
  return `${formatNumber(value)}${unit === "percent" ? "%" : ""}`;
}

function formatTargetRange(target: TargetStatRange) {
  const minimum = formatTargetValue(target.minimum, target.unit);
  const maximum = formatTargetValue(target.maximum, target.unit);

  if (minimum && maximum) return minimum === maximum ? minimum : `${minimum} ～ ${maximum}`;
  if (minimum) return `${minimum} 以上`;
  if (maximum) return `${maximum} 以下`;
  return "指定なし";
}

function formatConditionValue(value: BuildCondition["value"]) {
  if (typeof value === "boolean") return value ? "はい" : "いいえ";
  return String(value);
}

function formatCondition(condition: BuildCondition) {
  const operatorLabels: Record<BuildCondition["operator"], string> = {
    equals: "＝",
    not_equals: "≠",
    includes: "を含む",
    gte: "以上",
    lte: "以下",
  };
  return condition.description || `${condition.field} ${operatorLabels[condition.operator]} ${formatConditionValue(condition.value)}`;
}

function ArtifactPlanDetails({ plan }: { plan: ArtifactPlan }) {
  return (
    <dl className="grid gap-2 rounded-lg border border-slate-700 bg-slate-950/40 p-3">
      <div className="min-w-0">
        <dt className="text-sm text-slate-400">聖遺物セット</dt>
        <dd className="mt-1 break-words text-pretty font-semibold text-slate-100">{formatArtifactPlan(plan)}</dd>
      </div>
    </dl>
  );
}

function MainStatDetails({ mainStatPackage, headingId }: { mainStatPackage: MainStatPackage; headingId: string }) {
  return (
    <section className="space-y-3" aria-labelledby={headingId}>
      <h5 id={headingId} className="text-balance text-sm font-semibold text-slate-200">
        メインステータス（砂・杯・冠）
      </h5>
      <dl className="grid gap-2 sm:grid-cols-3">
        <div className="min-w-0 rounded-md border border-slate-700 bg-slate-950/40 p-3">
          <dt className="text-xs text-slate-400">砂</dt>
          <dd className="mt-1 break-words text-pretty text-sm font-semibold text-slate-100">{mainStatPackage.sands}</dd>
        </div>
        <div className="min-w-0 rounded-md border border-slate-700 bg-slate-950/40 p-3">
          <dt className="text-xs text-slate-400">杯</dt>
          <dd className="mt-1 break-words text-pretty text-sm font-semibold text-slate-100">{mainStatPackage.goblet}</dd>
        </div>
        <div className="min-w-0 rounded-md border border-slate-700 bg-slate-950/40 p-3">
          <dt className="text-xs text-slate-400">冠</dt>
          <dd className="mt-1 break-words text-pretty text-sm font-semibold text-slate-100">{mainStatPackage.circlet}</dd>
        </div>
      </dl>
    </section>
  );
}

function SubstatPriorityDetails({ priorities, headingId }: { priorities: StatPriority[]; headingId: string }) {
  return (
    <section className="space-y-2" aria-labelledby={headingId}>
      <h5 id={headingId} className="text-balance text-sm font-semibold text-slate-200">
        サブステータス優先度
      </h5>
      {priorities.length > 0 ? (
        <ol className="grid gap-2 sm:grid-cols-2">
          {priorities.map((priority) => (
            <li
              key={`${priority.rank}-${priority.stat}`}
              className="min-w-0 rounded-md border border-slate-700 bg-slate-950/40 p-2 text-sm text-slate-200"
            >
              <span className="tabular-nums text-slate-400">{priority.rank}位：</span>
              <span className="break-words">{priority.stat}</span>
            </li>
          ))}
        </ol>
      ) : (
        <p className="text-pretty text-sm leading-6 text-slate-400">サブステータスの指定はありません。</p>
      )}
    </section>
  );
}

function TargetStatDetails({ targets, headingId }: { targets: TargetStatRange[]; headingId: string }) {
  return (
    <section className="space-y-2" aria-labelledby={headingId}>
      <h5 id={headingId} className="text-balance text-sm font-semibold text-slate-200">
        目標ステータス
      </h5>
      {targets.length > 0 ? (
        <div className="overflow-x-auto rounded-lg border border-slate-700">
          <table className="min-w-full text-left text-sm">
            <thead className="bg-slate-950/70 text-xs text-slate-400">
              <tr>
                <th className="whitespace-nowrap px-3 py-2 font-medium">ステータス</th>
                <th className="whitespace-nowrap px-3 py-2 font-medium">目標</th>
                <th className="whitespace-nowrap px-3 py-2 font-medium">範囲</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-700">
              {targets.map((target) => (
                <tr key={`${target.stat}-${target.scope}-${target.minimum}-${target.maximum}`}>
                  <th className="min-w-0 break-words px-3 py-2 font-medium text-slate-200">{target.stat}</th>
                  <td className="whitespace-nowrap px-3 py-2 tabular-nums text-slate-100">{formatTargetRange(target)}</td>
                  <td className="min-w-0 break-words px-3 py-2 text-pretty text-slate-300">
                    {TARGET_SCOPE_LABELS[target.scope]}（{target.scope}）
                    {target.note ? <span className="block text-xs text-slate-400">{target.note}</span> : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="text-pretty text-sm leading-6 text-slate-400">目標ステータスの指定はありません。</p>
      )}
    </section>
  );
}

function EvidenceDetails({ claims, headingId }: { claims: EvidenceClaim[]; headingId: string }) {
  return (
    <section className="space-y-2" aria-labelledby={headingId}>
      <h5 id={headingId} className="text-balance text-sm font-semibold text-slate-200">
        根拠
      </h5>
      {claims.length > 0 ? (
        <ul className="space-y-3">
          {claims.map((claim, index) => (
            <li key={`${claim.claimType}-${claim.evidence.sourcePageId}-${index}`} className="min-w-0 rounded-lg border border-slate-700 bg-slate-950/40 p-3">
              <div className="flex min-w-0 flex-wrap gap-x-3 gap-y-1 text-xs text-slate-300">
                <span className="tabular-nums font-semibold text-amber-200">EvidenceGrade: {claim.evidenceGrade}</span>
                <span>（{EVIDENCE_GRADE_LABELS[claim.evidenceGrade]}）</span>
                <span className="break-words">claim: {EVIDENCE_CLAIM_LABELS[claim.claimType]}</span>
              </div>
              <p className="mt-1 break-words text-pretty text-xs text-slate-400">
                verification: {claim.evidence.verification}（{EVIDENCE_VERIFICATION_LABELS[claim.evidence.verification]}）
              </p>
              <p className="mt-2 break-words text-pretty text-sm leading-6 text-slate-200">{claim.evidence.evidenceSummary}</p>
              {claim.evidence.evidenceExcerpt ? (
                <blockquote className="mt-2 break-words whitespace-pre-wrap border-l-2 border-slate-600 pl-3 text-pretty text-xs leading-5 text-slate-400">
                  {claim.evidence.evidenceExcerpt}
                </blockquote>
              ) : null}
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-pretty text-sm leading-6 text-slate-400">表示できる根拠がありません。</p>
      )}
    </section>
  );
}

function ConditionsDetails({ conditions, headingId }: { conditions: BuildCondition[]; headingId: string }) {
  if (conditions.length === 0) return null;
  return (
    <section className="space-y-2" aria-labelledby={headingId}>
      <h5 id={headingId} className="text-balance text-sm font-semibold text-slate-200">
        適用条件
      </h5>
      <ul className="space-y-1 text-sm leading-6 text-slate-300">
        {conditions.map((condition, index) => (
          <li key={`${condition.field}-${condition.operator}-${index}`} className="break-words text-pretty">
            {formatCondition(condition)}
          </li>
        ))}
      </ul>
    </section>
  );
}

function VariantBreakdown({ variant, idPrefix }: { variant: BuildVariant; idPrefix: string }) {
  return (
    <div className="mt-3 space-y-4">
      <ArtifactPlanDetails plan={variant.artifactPlan} />
      <MainStatDetails mainStatPackage={variant.mainStatPackage} headingId={`${idPrefix}-main-stat`} />
      <SubstatPriorityDetails priorities={variant.mainStatPackage.substatPriority} headingId={`${idPrefix}-substat`} />
      <TargetStatDetails targets={variant.mainStatPackage.targetStats} headingId={`${idPrefix}-target`} />
      <ConditionsDetails conditions={variant.conditions} headingId={`${idPrefix}-condition`} />
      <EvidenceDetails claims={variant.evidenceClaims} headingId={`${idPrefix}-evidence`} />
    </div>
  );
}

function CandidateSummary({ variant }: { variant: BuildVariant }) {
  return (
    <span className="block min-w-0 text-left">
      <span className="block break-words font-semibold">候補 {variant.id}</span>
      <span className="mt-1 block break-words text-sm text-slate-300">{formatArtifactPlan(variant.artifactPlan)}</span>
      <span className="mt-1 block break-words text-xs text-slate-400">
        砂：{variant.mainStatPackage.sands} ／ 杯：{variant.mainStatPackage.goblet} ／ 冠：{variant.mainStatPackage.circlet}
      </span>
    </span>
  );
}

function ChoiceCandidate({
  characterId,
  variant,
  onChooseVariant,
}: {
  characterId: string;
  variant: BuildVariant;
  onChooseVariant?: (characterId: string, variantId: string) => void;
}) {
  return (
    <li className="min-w-0 rounded-lg border border-amber-300/50 bg-amber-400/5 p-3">
      <button
        type="button"
        className="min-h-11 w-full rounded-md border border-amber-300/60 px-3 py-2 text-amber-100 hover:bg-amber-300/10 focus-visible:outline-2 focus-visible:outline-amber-400 disabled:cursor-not-allowed disabled:opacity-60"
        onClick={() => onChooseVariant?.(characterId, variant.id)}
        disabled={!onChooseVariant}
        aria-label={`候補 ${variant.id} を選択`}
        data-testid="variant-choice"
        data-character-id={characterId}
        data-variant-id={variant.id}
      >
        <CandidateSummary variant={variant} />
      </button>
      <VariantBreakdown variant={variant} idPrefix={`choice-${characterId}-${variant.id}`} />
    </li>
  );
}

function AlternativesDetails({
  member,
  resolutionStatus,
  onChooseVariant,
}: {
  member: CharacterBuildResolution;
  resolutionStatus: TeamBuildResolution["status"];
  onChooseVariant?: (characterId: string, variantId: string) => void;
}) {
  const isChoicePending = resolutionStatus === "needs_user_choice" && member.selectedVariantId === null;

  if (member.alternatives.length === 0) {
    return <p className="mt-3 text-pretty text-sm leading-6 text-slate-400">候補はありません。</p>;
  }

  return (
    <section className="mt-4 space-y-2" aria-labelledby={`alternatives-${member.characterId}`}>
      <h4 id={`alternatives-${member.characterId}`} className="text-balance text-sm font-semibold text-slate-200">
        {isChoicePending ? "候補を選択" : "代替候補"}
      </h4>
      {isChoicePending ? (
        <ul className="space-y-3">
          {member.alternatives.map((variant) => (
            <ChoiceCandidate
              key={variant.id}
              characterId={member.characterId}
              variant={variant}
              onChooseVariant={onChooseVariant}
            />
          ))}
        </ul>
      ) : (
        <ul className="space-y-3">
          {member.alternatives
            .filter((variant) => variant.id !== member.selectedVariantId)
            .map((variant) => (
              <li key={variant.id} className="min-w-0 rounded-lg border border-slate-700 bg-slate-950/30 p-3">
                <h5 className="break-words text-sm font-semibold text-slate-200">候補 {variant.id}</h5>
                <VariantBreakdown variant={variant} idPrefix={`alternative-${member.characterId}-${variant.id}`} />
              </li>
            ))}
        </ul>
      )}
    </section>
  );
}

function CharacterResultCard({
  member,
  index,
  resolutionStatus,
  onChooseVariant,
}: {
  member: CharacterBuildResolution | null;
  index: number;
  resolutionStatus: TeamBuildResolution["status"];
  onChooseVariant?: (characterId: string, variantId: string) => void;
}) {
  if (!member) {
    return (
      <article
        className="min-w-72 rounded-xl border border-slate-700 bg-slate-900/60 p-4"
        aria-label={`キャラクター${index + 1}`}
        data-testid="character-result-card"
      >
        <h3 className="text-balance text-base font-semibold text-slate-200">キャラクター{index + 1}</h3>
        <p className="mt-3 text-pretty text-sm leading-6 text-slate-400">この枠の分析結果はありません。</p>
      </article>
    );
  }

  const selectedVariant = member.selectedVariantId
    ? member.alternatives.find((variant) => variant.id === member.selectedVariantId) ?? null
    : null;

  return (
    <article
      className="min-w-72 rounded-xl border border-slate-700 bg-slate-900/80 p-4"
      aria-labelledby={`member-heading-${index}`}
      data-testid="character-result-card"
    >
      <div className="flex min-w-0 flex-wrap items-start justify-between gap-2">
        <h3 id={`member-heading-${index}`} className="min-w-0 break-words text-balance text-lg font-bold text-slate-100">
          {member.characterId}
        </h3>
        {member.selectedVariantId ? (
          <span className="shrink-0 rounded-full border border-emerald-300/50 px-2 py-1 text-xs font-semibold text-emerald-200">
            選択済み
          </span>
        ) : (
          <span className="shrink-0 rounded-full border border-amber-300/50 px-2 py-1 text-xs font-semibold text-amber-200">
            未選択
          </span>
        )}
      </div>

      <section className="mt-4" aria-labelledby={`reason-${index}`}>
        <h4 id={`reason-${index}`} className="text-balance text-sm font-semibold text-slate-200">理由</h4>
        <p className="mt-1 break-words text-pretty text-sm leading-6 text-slate-300">{member.reason}</p>
      </section>

      {selectedVariant ? (
        <section className="mt-4" aria-labelledby={`selected-${index}`}>
          <h4 id={`selected-${index}`} className="text-balance text-sm font-semibold text-slate-200">
            選択中の候補：{selectedVariant.id}
          </h4>
          <VariantBreakdown variant={selectedVariant} idPrefix={`selected-${member.characterId}-${selectedVariant.id}`} />
        </section>
      ) : (
        <p className="mt-4 rounded-md border border-amber-300/40 bg-amber-400/5 p-3 text-pretty text-sm leading-6 text-amber-100">
          選択された候補はありません。
        </p>
      )}

      <AlternativesDetails
        member={member}
        resolutionStatus={resolutionStatus}
        onChooseVariant={onChooseVariant}
      />
    </article>
  );
}

function ResultValidityBanner({ validity }: { validity: ResultValidity }) {
  if (validity === "current") return null;
  return (
    <aside
      className="rounded-lg border border-amber-300/60 bg-amber-400/10 p-4"
      data-testid="team-result-stale-banner"
      aria-label="分析結果の有効性"
    >
      <p className="font-semibold text-amber-100">{VALIDITY_LABELS[validity]}</p>
      <p className="mt-1 break-words text-pretty text-sm leading-6 text-amber-200">
        この結果は最新の入力や根拠と一致しない可能性があります。表示内容を確認してから利用してください。
      </p>
    </aside>
  );
}

function WarningsDetails({ warnings }: { warnings: string[] }) {
  if (warnings.length === 0) return null;
  return (
    <section className="rounded-lg border border-amber-300/50 bg-amber-400/5 p-4" aria-labelledby="result-warnings-heading">
      <h3 id="result-warnings-heading" className="text-balance text-base font-semibold text-amber-100">注意</h3>
      <ul className="mt-2 space-y-2 text-pretty text-sm leading-6 text-amber-200">
        {warnings.map((warning, index) => (
          <li key={`${warning}-${index}`} className="break-words">{warning}</li>
        ))}
      </ul>
    </section>
  );
}

function EmptyResultState() {
  return (
    <div className="rounded-xl border border-dashed border-slate-600 bg-slate-900/60 p-6" data-testid="team-result-empty">
      <h3 className="text-balance text-lg font-semibold text-slate-100">分析結果はまだありません</h3>
      <p className="mt-2 break-words text-pretty text-sm leading-6 text-slate-300">
        編成を分析して、キャラクターごとの候補と根拠を表示してください。
      </p>
    </div>
  );
}

export function TeamResultPanel({ resolution, validity, onChooseVariant }: TeamResultPanelProps) {
  const members = resolution ? Array.from({ length: 4 }, (_, index) => resolution.members[index] ?? null) : [];

  return (
    <section className="space-y-4" aria-labelledby="team-result-heading" data-testid="team-result-panel">
      <div>
        <h2 id="team-result-heading" className="text-balance text-xl font-bold text-slate-100">チーム分析結果</h2>
        <p className="mt-1 text-pretty text-sm leading-6 text-slate-300">候補、目標値、根拠の検証状態をキャラクターごとに確認できます。</p>
      </div>

      <ResultValidityBanner validity={validity} />

      {resolution === null ? (
        <EmptyResultState />
      ) : (
        <>
          <div className="rounded-lg border border-slate-700 bg-slate-900/70 p-4" role="status" aria-live="polite">
            <p className={cn("text-pretty font-semibold", resolution.status === "unresolved" ? "text-rose-200" : "text-slate-100")}>
              {RESOLUTION_STATUS_LABELS[resolution.status]}
            </p>
            {resolution.status === "needs_user_choice" ? (
              <p className="mt-1 text-pretty text-sm leading-6 text-amber-200">未選択の候補ボタンから、各キャラクターの案を選んでください。</p>
            ) : null}
            {resolution.status === "unresolved" ? (
              <p className="mt-1 text-pretty text-sm leading-6 text-rose-200">条件を満たす組み合わせを確定できませんでした。注意事項と理由を確認してください。</p>
            ) : null}
          </div>

          <div className="overflow-x-auto pb-2" aria-label="4人分の分析結果">
            <div className="grid min-w-max grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
              {members.map((member, index) => (
                <CharacterResultCard
                  key={member?.characterId ?? `empty-${index}`}
                  member={member}
                  index={index}
                  resolutionStatus={resolution.status}
                  onChooseVariant={onChooseVariant}
                />
              ))}
            </div>
          </div>

          <WarningsDetails warnings={resolution.warnings} />
        </>
      )}
    </section>
  );
}
