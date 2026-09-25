import { FormEvent, useEffect, useRef, useState } from "react";
import {
  Bot,
  Check,
  ChevronRight,
  Clock3,
  Database,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  RefreshCw,
  Send,
  Settings2,
  ShieldCheck,
  Sparkles,
  Users,
} from "lucide-react";
import { cn } from "../../lib/cn";

type ResearchStage = "empty" | "clarifying" | "researching" | "result";
type ResultTab = "build" | "conversation";
type TargetStat = {
  label: string;
  value: string;
  primary?: boolean;
};

type TeamMember = {
  id: string;
  name: string;
  element: string;
  role: string;
  constellation: string;
  imageUrl: string;
  weapon: string;
  weaponImageUrl: string;
  artifact: string;
  artifactImageUrl: string;
  mainStats: string;
  subStats: string;
  targetStats: TargetStat[];
  accentClass: string;
  badgeClass: string;
};

const TEAM: TeamMember[] = [
  {
    id: "arlecchino",
    name: "アルレッキーノ",
    element: "炎",
    role: "メインアタッカー",
    constellation: "無凸前提",
    imageUrl: "https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Arlecchino.png",
    weapon: "赤月のシルエット",
    weaponImageUrl: "https://gi.yatta.moe/assets/UI/UI_EquipIcon_Pole_BloodMoon.png",
    artifact: "諧律奇想の断章",
    artifactImageUrl: "https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15035_4.png",
    mainStats: "攻撃力% / 炎元素ダメージ / 会心",
    subStats: "会心率 ＞ 会心ダメージ ＞ 攻撃力%",
    targetStats: [
      { label: "攻撃力", value: "2,000–2,300", primary: true },
      { label: "会心率", value: "70–80%" },
      { label: "会心ダメージ", value: "180%以上" },
      { label: "元素熟知", value: "100–150" },
      { label: "炎元素ダメージ", value: "46.6%" },
    ],
    accentClass: "border-rose-400/35",
    badgeClass: "bg-rose-400/10 text-rose-200",
  },
  {
    id: "yelan",
    name: "夜蘭",
    element: "水",
    role: "サブアタッカー",
    constellation: "無凸前提",
    imageUrl: "https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Yelan.png",
    weapon: "若水",
    weaponImageUrl: "https://gi.yatta.moe/assets/UI/UI_EquipIcon_Bow_Kirin.png",
    artifact: "絶縁の旗印",
    artifactImageUrl: "https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15020_4.png",
    mainStats: "元素チャージ / 水元素ダメージ / 会心",
    subStats: "元素チャージ ＞ 会心率 ＞ 会心ダメージ",
    targetStats: [
      { label: "HP", value: "32,000–36,000", primary: true },
      { label: "元素チャージ効率", value: "190–210%" },
      { label: "会心率", value: "70%以上" },
      { label: "水元素ダメージ", value: "46.6%" },
    ],
    accentClass: "border-sky-400/35",
    badgeClass: "bg-sky-400/10 text-sky-200",
  },
  {
    id: "bennett",
    name: "ベネット",
    element: "炎",
    role: "攻撃支援・回復",
    constellation: "無凸前提",
    imageUrl: "https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Bennett.png",
    weapon: "原木刀",
    weaponImageUrl: "https://gi.yatta.moe/assets/UI/UI_EquipIcon_Sword_Arakalari.png",
    artifact: "旧貴族のしつけ",
    artifactImageUrl: "https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15007_4.png",
    mainStats: "元素チャージ / HP% / 治療効果",
    subStats: "元素チャージ ＞ HP% ＞ HP",
    targetStats: [
      { label: "基礎攻撃力", value: "756", primary: true },
      { label: "元素チャージ効率", value: "230%以上" },
      { label: "HP", value: "24,000以上" },
      { label: "治療効果", value: "35.9%" },
    ],
    accentClass: "border-orange-400/35",
    badgeClass: "bg-orange-400/10 text-orange-200",
  },
  {
    id: "zhongli",
    name: "鍾離",
    element: "岩",
    role: "シールド・耐性低下",
    constellation: "無凸前提",
    imageUrl: "https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Zhongli.png",
    weapon: "西風長槍",
    weaponImageUrl: "https://gi.yatta.moe/assets/UI/UI_EquipIcon_Pole_Zephyrus.png",
    artifact: "千岩牢固",
    artifactImageUrl: "https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15017_4.png",
    mainStats: "HP% / HP% / HP%",
    subStats: "HP% ＞ HP ＞ 会心率",
    targetStats: [
      { label: "HP", value: "45,000以上", primary: true },
      { label: "会心率", value: "45%以上" },
      { label: "元素チャージ効率", value: "140–160%" },
    ],
    accentClass: "border-amber-400/35",
    badgeClass: "bg-amber-400/10 text-amber-200",
  },
];

const EXAMPLE_PROMPT = "アルレッキーノ、夜蘭、ベネット、鍾離の4人を調べたい。";

function Portrait({ member, size = "large" }: { member: TeamMember; size?: "small" | "large" }) {
  return (
    <div
      className={cn(
        "relative shrink-0 overflow-hidden rounded-xl border border-white/10 bg-slate-800",
        size === "large" ? "size-16" : "size-10",
      )}
    >
      <span className="absolute inset-0 grid place-items-center text-sm font-bold text-slate-500" aria-hidden="true">
        {member.name.slice(0, 1)}
      </span>
      <img className="relative size-full object-cover" src={member.imageUrl} alt={`${member.name}のアイコン`} />
    </div>
  );
}

function EmptyConversation({ onUseExample }: { onUseExample: () => void }) {
  return (
    <section className="mx-auto flex w-full max-w-3xl flex-1 flex-col items-center justify-center px-5 py-12 text-center">
      <div className="grid size-14 place-items-center rounded-2xl border border-amber-300/25 bg-amber-300/10 text-amber-300">
        <Sparkles aria-hidden="true" size={26} />
      </div>
      <p className="mt-6 text-sm font-semibold text-amber-300">新しい編成調査</p>
      <h1 className="mt-2 text-balance text-3xl font-bold tracking-tight text-slate-50 sm:text-4xl">
        調べたい4人を教えてください
      </h1>
      <p className="mt-4 max-w-xl text-pretty leading-7 text-slate-400">
        4人の名前を送ると、Codexが不足している武器や凸数だけを確認します。情報が揃った編成だけ、この端末へ保存します。
      </p>
      <button
        type="button"
        className="mt-8 max-w-xl rounded-xl border border-slate-700 bg-slate-900 px-4 py-3 text-left text-sm leading-6 text-slate-300 hover:border-slate-500 hover:bg-slate-800"
        onClick={onUseExample}
      >
        <span className="block text-xs font-semibold text-slate-500">入力例</span>
        <span className="mt-1 block">{EXAMPLE_PROMPT}</span>
      </button>
    </section>
  );
}

function MemberStrip() {
  return (
    <div className="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="確認した4人">
      {TEAM.map((member, index) => (
        <div key={member.id} className="flex min-w-0 items-center gap-3 rounded-xl border border-slate-700 bg-slate-950/60 p-3">
          <Portrait member={member} size="small" />
          <div className="min-w-0">
            <p className="text-xs text-slate-500">スロット {index + 1}</p>
            <p className="truncate font-semibold text-slate-100">{member.name}</p>
          </div>
        </div>
      ))}
    </div>
  );
}

function ClarifyingConversation({ prompt, onStartResearch }: { prompt: string; onStartResearch: () => void }) {
  return (
    <section className="mx-auto w-full max-w-4xl flex-1 px-5 py-8 sm:px-8">
      <div className="flex justify-end">
        <div className="max-w-2xl rounded-2xl rounded-br-md bg-amber-300 px-4 py-3 text-sm leading-6 text-slate-950">
          {prompt}
        </div>
      </div>

      <div className="mt-7 flex items-start gap-3">
        <div className="grid size-9 shrink-0 place-items-center rounded-xl border border-slate-700 bg-slate-800 text-amber-300">
          <Bot aria-hidden="true" size={18} />
        </div>
        <div className="min-w-0 flex-1 rounded-2xl rounded-tl-md border border-slate-700 bg-slate-900 p-5">
          <p className="font-semibold text-slate-100">4人を確認しました</p>
          <p className="mt-2 text-pretty text-sm leading-6 text-slate-400">
            武器と凸数がまだ分かりません。チャットで追加するか、未指定のまま候補を調査できます。
          </p>
          <MemberStrip />
          <div className="mt-5 rounded-xl border-l-2 border-amber-300 bg-amber-300/5 px-4 py-3 text-sm leading-6 text-slate-300">
            このまま始めると、アルレッキーノを主軸にした蒸発編成として調査します。
          </div>
          <div className="mt-5 flex flex-wrap gap-3">
            <button
              type="button"
              className="inline-flex min-h-11 items-center gap-2 rounded-xl bg-amber-300 px-4 py-2 font-semibold text-slate-950 hover:bg-amber-200"
              onClick={onStartResearch}
            >
              この内容で調査する
              <ChevronRight aria-hidden="true" size={17} />
            </button>
            <span className="inline-flex min-h-11 items-center text-sm text-slate-500">変更する場合は下のチャットへ入力</span>
          </div>
        </div>
      </div>
    </section>
  );
}

function ResearchingConversation() {
  const steps = [
    { label: "4人の基本情報", state: "完了" },
    { label: "武器・聖遺物の候補", state: "調査中" },
    { label: "編成内の効果と目標値", state: "待機中" },
  ];

  return (
    <section className="mx-auto flex w-full max-w-3xl flex-1 flex-col justify-center px-5 py-12">
      <div className="rounded-2xl border border-slate-700 bg-slate-900 p-6 sm:p-8" role="status" aria-live="polite">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="text-sm font-semibold text-amber-300">Codexが調査中</p>
            <h1 className="mt-2 text-balance text-2xl font-bold text-slate-50">4人の情報を集めています</h1>
          </div>
          <span className="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-400">通常 1〜3分</span>
        </div>
        <div className="mt-7 flex -space-x-2" aria-label="調査対象の4人">
          {TEAM.map((member) => (
            <div key={member.id} className="rounded-xl border-2 border-slate-900">
              <Portrait member={member} size="small" />
            </div>
          ))}
        </div>
        <ol className="mt-7 divide-y divide-slate-800 border-y border-slate-800">
          {steps.map((step) => (
            <li key={step.label} className="flex items-center justify-between gap-4 py-4">
              <span className="text-sm text-slate-300">{step.label}</span>
              <span
                className={cn(
                  "inline-flex items-center gap-2 text-xs font-semibold",
                  step.state === "完了" ? "text-emerald-300" : step.state === "調査中" ? "text-amber-300" : "text-slate-500",
                )}
              >
                {step.state === "完了" ? <Check aria-hidden="true" size={15} /> : <Clock3 aria-hidden="true" size={15} />}
                {step.state}
              </span>
            </li>
          ))}
        </ol>
        <p className="mt-5 text-pretty text-sm leading-6 text-slate-500">
          この画面を閉じても調査は続きます。完了した編成だけ保存されます。
        </p>
      </div>
    </section>
  );
}

function BuildCard({ member, index }: { member: TeamMember; index: number }) {
  return (
    <article className={cn("min-w-0 overflow-hidden rounded-2xl border bg-slate-900", member.accentClass)}>
      <div className="flex items-start gap-4 border-b border-slate-800 p-5">
        <Portrait member={member} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-semibold text-slate-500">#{index + 1}</span>
            <span className={cn("rounded-md px-2 py-0.5 text-xs font-semibold", member.badgeClass)}>{member.element}</span>
          </div>
          <h3 className="mt-1 text-balance text-xl font-bold text-slate-50">{member.name}</h3>
          <p className="mt-1 text-sm text-slate-400">{member.constellation}・{member.role}</p>
        </div>
      </div>
      <div className="space-y-5 p-5">
        <div className="grid grid-cols-[44px_1fr] items-center gap-3">
          <img className="size-11 rounded-lg bg-slate-800 object-cover" src={member.weaponImageUrl} alt="" />
          <div className="min-w-0">
            <p className="text-xs text-slate-500">おすすめ武器</p>
            <p className="truncate text-sm font-semibold text-slate-200">{member.weapon}</p>
          </div>
        </div>
        <div className="grid grid-cols-[44px_1fr] items-center gap-3">
          <img className="size-11 rounded-lg bg-slate-800 object-cover" src={member.artifactImageUrl} alt="" />
          <div className="min-w-0">
            <p className="text-xs text-slate-500">おすすめ聖遺物</p>
            <p className="truncate text-sm font-semibold text-slate-200">{member.artifact}</p>
          </div>
        </div>
        <dl className="space-y-3 border-t border-slate-800 pt-4 text-sm">
          <div>
            <dt className="text-xs text-slate-500">メインステータス</dt>
            <dd className="mt-1 break-words leading-6 text-slate-300">{member.mainStats}</dd>
          </div>
          <div>
            <dt className="text-xs text-slate-500">サブステータス優先度</dt>
            <dd className="mt-1 break-words leading-6 text-slate-300">{member.subStats}</dd>
          </div>
        </dl>
        <div className="border-t border-slate-800 pt-4">
          <div className="flex items-center justify-between gap-3">
            <h4 className="text-sm font-semibold text-slate-200">目標ステータス</h4>
            <span className="text-xs text-slate-500">戦闘前の目安</span>
          </div>
          <dl className="mt-3 grid grid-cols-2 gap-2">
            {member.targetStats.map((target) => (
              <div
                key={target.label}
                className={cn(
                  "min-w-0 rounded-lg border px-3 py-2.5",
                  target.primary
                    ? "border-amber-300/35 bg-amber-300/10"
                    : "border-slate-800 bg-slate-950/50",
                )}
              >
                <dt className={cn("flex flex-wrap items-center gap-1.5 text-xs", target.primary ? "text-amber-200" : "text-slate-500")}>
                  {target.label}
                  {target.primary ? (
                    <span className="rounded bg-amber-300/15 px-1.5 py-0.5 text-[10px] font-semibold">主参照</span>
                  ) : null}
                </dt>
                <dd className="mt-1 break-words text-sm font-bold tabular-nums text-slate-100">{target.value}</dd>
              </div>
            ))}
          </dl>
        </div>
      </div>
    </article>
  );
}

function ResultView({ tab, onTabChange, onRevise }: { tab: ResultTab; onTabChange: (tab: ResultTab) => void; onRevise: () => void }) {
  return (
    <section className="mx-auto w-full max-w-6xl flex-1 px-5 py-7 sm:px-8">
      <div className="flex flex-wrap items-start justify-between gap-5">
        <div>
          <div className="flex flex-wrap items-center gap-2 text-xs font-semibold text-emerald-300">
            <ShieldCheck aria-hidden="true" size={16} />
            調査済み・この端末に保存
          </div>
          <h1 className="mt-2 text-balance text-3xl font-bold tracking-tight text-slate-50">アルレッキーノ蒸発編成</h1>
          <p className="mt-2 text-sm text-slate-400">Ver.7.0を対象に、12件の本文を確認しました。</p>
          <p className="mt-1 max-w-2xl text-pretty text-xs leading-5 text-slate-500">
            会心やダメージバフに加え、攻撃力・HP・防御力・元素熟知など、役割の土台になる数値も算出しています。
          </p>
        </div>
        <button
          type="button"
          className="inline-flex min-h-11 items-center gap-2 rounded-xl border border-slate-700 px-4 py-2 text-sm font-semibold text-slate-200 hover:bg-slate-800"
          onClick={onRevise}
        >
          <RefreshCw aria-hidden="true" size={16} />
          条件を変えて再調査
        </button>
      </div>

      <div className="mt-7 flex gap-1 border-b border-slate-800" role="tablist" aria-label="編成の表示内容">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "build"}
          className={cn(
            "min-h-11 border-b-2 px-4 text-sm font-semibold",
            tab === "build" ? "border-amber-300 text-amber-200" : "border-transparent text-slate-500 hover:text-slate-300",
          )}
          onClick={() => onTabChange("build")}
        >
          ビルド
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "conversation"}
          className={cn(
            "min-h-11 border-b-2 px-4 text-sm font-semibold",
            tab === "conversation" ? "border-amber-300 text-amber-200" : "border-transparent text-slate-500 hover:text-slate-300",
          )}
          onClick={() => onTabChange("conversation")}
        >
          会話と調査メモ
        </button>
      </div>

      {tab === "build" ? (
        <div className="mt-6 grid gap-4 lg:grid-cols-2">
          {TEAM.map((member, index) => <BuildCard key={member.id} member={member} index={index} />)}
        </div>
      ) : (
        <div className="mt-6 max-w-3xl space-y-5">
          <div className="ml-auto max-w-2xl rounded-2xl rounded-br-md bg-amber-300 px-4 py-3 text-sm leading-6 text-slate-950">
            {EXAMPLE_PROMPT}
          </div>
          <div className="flex items-start gap-3">
            <div className="grid size-9 shrink-0 place-items-center rounded-xl border border-slate-700 bg-slate-800 text-amber-300">
              <Bot aria-hidden="true" size={18} />
            </div>
            <div className="rounded-2xl rounded-tl-md border border-slate-700 bg-slate-900 p-5 text-sm leading-6 text-slate-300">
              4人の役割、武器候補、聖遺物、目標ステータスを確認しました。結果は「ビルド」タブへ整理して保存しています。
            </div>
          </div>
          <div className="rounded-xl border border-slate-800 bg-slate-900/60 p-4">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-300">
              <Database aria-hidden="true" size={16} />
              保存した調査情報
            </div>
            <p className="mt-2 text-sm leading-6 text-slate-500">キャラクター4件・武器4件・聖遺物4件・確認済み本文12件</p>
          </div>
        </div>
      )}
    </section>
  );
}

function Sidebar({
  open,
  saved,
  showingSaved,
  onClose,
  onNew,
  onOpenSaved,
}: {
  open: boolean;
  saved: boolean;
  showingSaved: boolean;
  onClose: () => void;
  onNew: () => void;
  onOpenSaved: () => void;
}) {
  if (!open) return null;

  return (
    <aside className="fixed inset-y-0 left-0 z-30 flex w-[19rem] flex-col border-r border-slate-800 bg-slate-950 p-4 lg:static" aria-label="編成ナビゲーション">
      <div className="flex items-center justify-between gap-3 px-1">
        <div className="flex items-center gap-3">
          <div className="grid size-9 place-items-center rounded-xl bg-amber-300 text-slate-950"><Sparkles aria-hidden="true" size={18} /></div>
          <div>
            <p className="font-bold text-slate-100">編成ノート</p>
            <p className="text-xs text-slate-500">Codexで調べて保存</p>
          </div>
        </div>
        <button type="button" className="grid size-10 place-items-center rounded-lg text-slate-400 hover:bg-slate-800 hover:text-slate-100" onClick={onClose} aria-label="サイドバーを閉じる">
          <PanelLeftClose aria-hidden="true" size={18} />
        </button>
      </div>

      <button
        type="button"
        className="mt-6 inline-flex min-h-11 items-center justify-center gap-2 rounded-xl bg-slate-100 px-4 py-2 font-semibold text-slate-950 hover:bg-white"
        onClick={onNew}
      >
        <Plus aria-hidden="true" size={18} />
        新しい編成を調べる
      </button>

      <div className="mt-7 flex items-center justify-between px-2">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-slate-500">保存した編成</h2>
        {saved ? <span className="text-xs tabular-nums text-slate-600">1件</span> : null}
      </div>

      <div className="mt-3 min-h-0 flex-1 overflow-y-auto">
        {saved ? (
          <button
            type="button"
            className={cn(
              "flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left",
              showingSaved ? "bg-slate-800 text-slate-100" : "text-slate-400 hover:bg-slate-900 hover:text-slate-200",
            )}
            onClick={onOpenSaved}
            aria-current={showingSaved ? "page" : undefined}
          >
            <div className="flex -space-x-2">
              {TEAM.slice(0, 3).map((member) => (
                <img key={member.id} className="size-8 rounded-lg border-2 border-slate-900 bg-slate-800 object-cover" src={member.imageUrl} alt="" />
              ))}
            </div>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-sm font-semibold">アルレッキーノ蒸発編成</span>
              <span className="mt-0.5 block text-xs text-slate-500">たった今更新</span>
            </span>
          </button>
        ) : (
          <div className="rounded-xl border border-dashed border-slate-800 px-4 py-5 text-center">
            <Users className="mx-auto text-slate-700" aria-hidden="true" size={21} />
            <p className="mt-2 text-xs leading-5 text-slate-600">調査が完了した編成が<br />ここに追加されます</p>
          </div>
        )}
      </div>

      <div className="border-t border-slate-800 pt-4">
        <button type="button" className="flex min-h-11 w-full items-center gap-3 rounded-xl px-3 text-sm text-slate-400 hover:bg-slate-900 hover:text-slate-200">
          <Settings2 aria-hidden="true" size={17} />
          Codex接続と設定
        </button>
      </div>
    </aside>
  );
}

function Composer({ value, onChange, onSubmit, disabled }: { value: string; onChange: (value: string) => void; onSubmit: () => void; disabled: boolean }) {
  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    onSubmit();
  };

  return (
    <div className="border-t border-slate-800 bg-slate-950/95 px-4 py-4 sm:px-8">
      <form className="mx-auto flex max-w-4xl items-end gap-3 rounded-2xl border border-slate-700 bg-slate-900 p-2 focus-within:border-amber-300/70" onSubmit={handleSubmit}>
        <label htmlFor="team-request" className="sr-only">調べたい編成</label>
        <textarea
          id="team-request"
          rows={2}
          className="max-h-36 min-h-12 min-w-0 flex-1 resize-none bg-transparent px-3 py-2 text-sm leading-6 text-slate-100 placeholder:text-slate-600 focus:outline-none disabled:cursor-not-allowed"
          placeholder="例：アルレッキーノ、夜蘭、ベネット、鍾離を調べたい"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          disabled={disabled}
        />
        <button
          type="submit"
          className="grid size-11 shrink-0 place-items-center rounded-xl bg-amber-300 text-slate-950 hover:bg-amber-200 disabled:cursor-not-allowed disabled:bg-slate-700 disabled:text-slate-500"
          disabled={disabled || value.trim().length === 0}
          aria-label="送信"
        >
          <Send aria-hidden="true" size={18} />
        </button>
      </form>
      <p className="mx-auto mt-2 max-w-4xl text-center text-xs text-slate-600">調査結果はCodexの回答を検証してから保存します</p>
    </div>
  );
}

export function ResearchChatPrototype() {
  const [stage, setStage] = useState<ResearchStage>("empty");
  const [composer, setComposer] = useState("");
  const [submittedPrompt, setSubmittedPrompt] = useState("");
  const [sidebarOpen, setSidebarOpen] = useState(() => (
    typeof window.matchMedia === "function" ? window.matchMedia("(min-width: 1024px)").matches : true
  ));
  const [saved, setSaved] = useState(false);
  const [resultTab, setResultTab] = useState<ResultTab>("build");
  const composerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (stage !== "researching") return;
    const timer = window.setTimeout(() => {
      setSaved(true);
      setResultTab("build");
      setStage("result");
    }, 1800);
    return () => window.clearTimeout(timer);
  }, [stage]);

  const handleSubmit = () => {
    const nextPrompt = composer.trim();
    if (!nextPrompt || stage === "researching") return;
    setSubmittedPrompt(nextPrompt);
    setComposer("");
    setStage("clarifying");
  };

  const handleNew = () => {
    setStage("empty");
    setComposer("");
    setSubmittedPrompt("");
    setResultTab("build");
  };

  const handleRevise = () => {
    setResultTab("conversation");
    setComposer("武器を星4だけにして、もう一度調べて");
    window.setTimeout(() => composerRef.current?.querySelector("textarea")?.focus(), 0);
  };

  const showingSaved = stage === "result";

  return (
    <div className="flex min-h-dvh bg-slate-950 text-slate-100">
      {sidebarOpen ? <button type="button" className="fixed inset-0 z-20 bg-black/60 lg:hidden" aria-label="サイドバーを閉じる" onClick={() => setSidebarOpen(false)} /> : null}
      <Sidebar
        open={sidebarOpen}
        saved={saved}
        showingSaved={showingSaved}
        onClose={() => setSidebarOpen(false)}
        onNew={handleNew}
        onOpenSaved={() => { setStage("result"); setResultTab("build"); }}
      />

      <main className="flex min-h-dvh min-w-0 flex-1 flex-col">
        <header className="flex min-h-16 items-center justify-between gap-4 border-b border-slate-800 px-4 sm:px-6">
          <div className="flex min-w-0 items-center gap-3">
            {!sidebarOpen ? (
              <button type="button" className="grid size-10 shrink-0 place-items-center rounded-lg text-slate-400 hover:bg-slate-800 hover:text-slate-100" onClick={() => setSidebarOpen(true)} aria-label="サイドバーを開く">
                <PanelLeftOpen aria-hidden="true" size={19} />
              </button>
            ) : null}
            <div className="min-w-0">
              <p className="truncate text-sm font-semibold text-slate-200">{stage === "result" ? "アルレッキーノ蒸発編成" : "新しい編成調査"}</p>
              <p className="truncate text-xs text-slate-600">4人指定で調査</p>
            </div>
          </div>
          <div className="flex items-center gap-2 rounded-full border border-slate-800 px-3 py-1.5 text-xs text-slate-400">
            <span className="size-2 rounded-full bg-emerald-400" aria-hidden="true" />
            Codex 接続済み
          </div>
        </header>

        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
          {stage === "empty" ? <EmptyConversation onUseExample={() => setComposer(EXAMPLE_PROMPT)} /> : null}
          {stage === "clarifying" ? <ClarifyingConversation prompt={submittedPrompt} onStartResearch={() => setStage("researching")} /> : null}
          {stage === "researching" ? <ResearchingConversation /> : null}
          {stage === "result" ? <ResultView tab={resultTab} onTabChange={setResultTab} onRevise={handleRevise} /> : null}
        </div>

        <div ref={composerRef}>
          <Composer value={composer} onChange={setComposer} onSubmit={handleSubmit} disabled={stage === "researching"} />
        </div>
      </main>
    </div>
  );
}

export default ResearchChatPrototype;
