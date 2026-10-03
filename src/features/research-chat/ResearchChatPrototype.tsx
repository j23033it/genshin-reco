import {
  FormEvent,
  RefObject,
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import {
  Bot,
  ChevronRight,
  Database,
  PanelLeftClose,
  PanelLeftOpen,
  Pencil,
  Plus,
  RefreshCw,
  Send,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import { cn } from "../../lib/cn";
import { OperationProgress } from "../../components/OperationProgress";
import { AppUpdateControl } from "./AppUpdateControl";
import { ResearchProgressPanel } from "./ResearchProgressPanel";
import { ResearchConditionsEditor } from "./ResearchConditionsEditor";
import { researchRepository } from "./researchRepository";
import { useResearchChat } from "./useResearchChat";
import type {
  GameId,
  ResearchConversation,
  ResearchMember,
  ResearchRepository,
  ResearchedTeamRecord,
  TeamMember,
} from "./types";

type ResultTab = "build" | "conversation";
function subscribeViewport(callback: () => void) {
  const query = window.matchMedia?.("(min-width: 1024px)");
  query?.addEventListener("change", callback);
  return () => query?.removeEventListener("change", callback);
}
function wideViewport() {
  return window.matchMedia?.("(min-width: 1024px)").matches ?? true;
}

const EXAMPLE_PROMPT = "アルレッキーノ、夜蘭、ベネット、鍾離の4人を調べたい。";
const actionClass =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-xl bg-amber-300 px-4 py-2 font-semibold text-slate-950 hover:bg-amber-200 disabled:cursor-not-allowed disabled:opacity-50";

function AssetImage({
  src,
  label,
  large = false,
}: {
  src?: string | null;
  label: string;
  large?: boolean;
}) {
  const [failedSrc, setFailedSrc] = useState<string | null>(null);
  return (
    <div
      className={cn(
        "relative grid shrink-0 place-items-center overflow-hidden rounded-xl border border-white/10 bg-slate-800 text-slate-400",
        large ? "size-16" : "size-11",
      )}
    >
      <span aria-hidden="true">{label.slice(0, 1) || "?"}</span>
      {src && failedSrc !== src ? (
        <img
          className="absolute inset-0 size-full object-cover"
          src={src}
          alt={label}
          onError={() => setFailedSrc(src)}
        />
      ) : (
        <span className="sr-only">{label}（画像なし）</span>
      )}
    </div>
  );
}

function Portrait({
  member,
  size = "large",
}: {
  member: ResearchMember;
  size?: "small" | "large";
}) {
  return (
    <AssetImage
      src={member.imageUrl}
      label={member.name}
      large={size === "large"}
    />
  );
}

function EmptyConversation({ onUseExample, game }: { onUseExample: () => void; game: GameId }) {
  return (
    <section className="mx-auto flex w-full max-w-3xl flex-1 flex-col items-center justify-center px-5 py-12 text-center">
      <div className="grid size-14 place-items-center rounded-2xl border border-amber-300/25 bg-amber-300/10 text-amber-300">
        <Sparkles aria-hidden="true" size={26} />
      </div>
      <p className="mt-6 text-sm font-semibold text-amber-300">
        新しい編成調査
      </p>
      <h1 className="mt-2 text-balance text-3xl font-bold tracking-tight text-slate-50 sm:text-4xl">
        調べたい4人を教えてください
      </h1>
      <p className="mt-4 max-w-xl text-pretty leading-7 text-slate-400">
        {game === "genshin" ? "4人の名前を送った後、凸と武器を画面で選べます。武器が決まっていなくても調査できます。" : "4人の名前を送った後、星魂・光円錐・重畳と任意の固定遺物を選べます。指定なしでも調査できます。"}
      </p>
      <button
        type="button"
        className="mt-8 max-w-xl rounded-xl border border-slate-700 bg-slate-900 px-4 py-3 text-left text-sm leading-6 text-slate-300 hover:border-slate-500 hover:bg-slate-800"
        onClick={onUseExample}
      >
        <span className="block text-xs font-semibold text-slate-500">
          入力例
        </span>
        <span className="mt-1 block">{game === "genshin" ? EXAMPLE_PROMPT : "ホタル、ルアン・メェイ、開拓者・調和、ギャラガーの4人を調べたい。"}</span>
      </button>
    </section>
  );
}

function Conversation({
  conversation,
}: {
  conversation: ResearchConversation;
}) {
  return (
    <div className="space-y-5" aria-label="調査の会話">
      {conversation.messages.map((message, index) => (
        <div
          key={index}
          className={cn(
            "flex items-start gap-3",
            message.role === "user" && "justify-end",
          )}
        >
          {message.role === "assistant" ? (
            <Bot
              className="mt-3 shrink-0 text-amber-300"
              size={20}
              aria-hidden="true"
            />
          ) : null}
          <div
            className={cn(
              "max-w-2xl min-w-0 whitespace-pre-wrap break-words rounded-2xl px-4 py-3 text-sm leading-7",
              message.role === "user"
                ? "bg-amber-300 text-slate-950"
                : "border border-slate-700 bg-slate-900 text-slate-200",
            )}
          >
            <span className="sr-only">
              {message.role === "user" ? "あなた" : "Codex"}：
            </span>
            {message.content}
          </div>
        </div>
      ))}
      {conversation.members.length ? (
        <div
          className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4"
          aria-label="確認したメンバー"
        >
          {conversation.members.map((member, index) => (
            <div
              key={`${member.slotIndex}-${index}`}
              className="flex min-w-0 items-center gap-3 rounded-xl border border-slate-700 p-3"
            >
              <Portrait
                member={{ id: String(member.slotIndex), name: member.name }}
                size="small"
              />
              <span className="min-w-0 break-words text-sm">{member.name}</span>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}

const elementStyles: Record<string, { border: string; badge: string }> = {
  炎: { border: "border-rose-400/35", badge: "bg-rose-400/10 text-rose-200" },
  水: { border: "border-sky-400/35", badge: "bg-sky-400/10 text-sky-200" },
  岩: {
    border: "border-amber-400/35",
    badge: "bg-amber-400/10 text-amber-200",
  },
  風: { border: "border-teal-400/35", badge: "bg-teal-400/10 text-teal-200" },
  雷: {
    border: "border-violet-400/35",
    badge: "bg-violet-400/10 text-violet-200",
  },
  氷: { border: "border-cyan-400/35", badge: "bg-cyan-400/10 text-cyan-200" },
  草: { border: "border-lime-400/35", badge: "bg-lime-400/10 text-lime-200" },
};

function RelicResult({ member, fixed }: { member: TeamMember; fixed?: import("./types").ResearchMemberInput }) {
  const build = member.starRail!;
  return <div className="space-y-4">
    <p className="text-xs leading-5 text-slate-400">実測ステータスは未入力です。発動条件は目標であり、達成を保証しません。</p>
    {[{ label: build.tunnel.kind === "four_piece" ? "トンネル遺物・4セット" : "トンネル遺物・2＋2", pieces: build.tunnel.kind === "four_piece" ? 4 : 2, fixed: Boolean(fixed?.relics?.tunnel), evidence: build.tunnelEvidence },
      { label: "オーナメント・2セット", pieces: 2, fixed: Boolean(fixed?.relics?.ornament), evidence: [build.ornamentEvidence] }].map(group => <div key={group.label} className="min-w-0 space-y-2">
        <p className="text-sm text-slate-300">{group.label} <span className="rounded bg-amber-300/10 px-2 py-1 text-xs text-amber-200">{group.fixed ? "指定を維持" : "自動提案"}</span></p>
        {group.evidence.map(evidence => <div key={evidence.set} className="min-w-0 space-y-2">
          <div className="flex items-center gap-3"><AssetImage src={evidence.imageUrl} label={evidence.set} /><p className="min-w-0 break-words text-sm font-semibold">{evidence.set}（{group.pieces}セット）</p></div>
          <p className="break-words text-xs leading-5 text-slate-300">採用理由：{evidence.reason}</p>
          <p className="break-words text-xs leading-5 text-slate-400">発動条件・注意点：{evidence.conditions}</p>
          {evidence.sourceUrls.map((url, index) => <a className="mr-3 inline-block break-all text-xs text-amber-200 underline" key={url} href={url} target="_blank" rel="noreferrer">装備の出典 {index + 1}</a>)}
        </div>)}
      </div>)}
  </div>;
}

function BuildCard({ member, index, fixed }: { member: TeamMember; index: number; fixed?: import("./types").ResearchMemberInput }) {
  return (
    <article
      className={cn(
        "min-w-0 overflow-hidden rounded-2xl border bg-slate-900",
        elementStyles[member.element]?.border ?? "border-slate-700",
      )}
    >
      <div className="flex items-start gap-4 border-b border-slate-800 p-5">
        <Portrait member={member} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-semibold text-slate-500">
              #{index + 1}
            </span>
            <span
              className={cn(
                "rounded-md px-2 py-0.5 text-xs font-semibold",
                elementStyles[member.element]?.badge ??
                  "bg-amber-300/10 text-amber-200",
              )}
            >
              {member.element}
            </span>
          </div>
          <h3 className="mt-1 text-balance text-xl font-bold text-slate-50">
            {member.name}
          </h3>
          <p className="mt-1 text-sm text-slate-400">
            {member.constellation}・{member.role}
          </p>
        </div>
      </div>
      <div className="space-y-5 p-5">
        <div className="grid grid-cols-[44px_1fr] items-center gap-3">
          <AssetImage src={member.weaponImageUrl} label={member.weapon} />
          <div className="min-w-0">
            <p className="text-xs text-slate-500">{member.starRail ? `光円錐・S${member.starRail.superimposition}` : "おすすめ武器"}</p>
            <p className="break-words text-sm font-semibold text-slate-200">
              {member.weapon}
            </p>
          </div>
        </div>
        {member.starRail ? <RelicResult member={member} fixed={fixed} /> : <div className="grid grid-cols-[44px_1fr] items-center gap-3">
          <AssetImage src={member.artifactImageUrl} label={member.artifact} />
          <div className="min-w-0">
            <p className="text-xs text-slate-500">おすすめ聖遺物</p>
            <p className="break-words text-sm font-semibold text-slate-200">
              {member.artifact}
            </p>
          </div>
        </div>
        }
        <dl className="space-y-3 border-t border-slate-800 pt-4 text-sm">
          <div>
            <dt className="text-xs text-slate-500">メインステータス</dt>
            <dd className="mt-1 break-words leading-6 text-slate-300">
              {member.mainStats || "未確認"}
            </dd>
          </div>
          <div>
            <dt className="text-xs text-slate-500">サブステータス優先度</dt>
            <dd className="mt-1 break-words leading-6 text-slate-300">
              {member.subStats || "未確認"}
            </dd>
          </div>
        </dl>
        <div className="border-t border-slate-800 pt-4">
          <div className="flex items-center justify-between gap-3">
            <h4 className="text-sm font-semibold text-slate-200">
              目標ステータス
            </h4>
            <span className="text-xs text-slate-500">戦闘前の目安</span>
          </div>
          {!member.targetStats?.length ? (
            <p className="mt-3 text-sm text-slate-400">目標値は未確認です。</p>
          ) : null}
          <dl className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">
            {(member.targetStats ?? []).map((target) => (
              <div
                key={target.label}
                className={cn(
                  "min-w-0 rounded-lg border px-3 py-2.5",
                  target.primary
                    ? "border-amber-300/35 bg-amber-300/10"
                    : "border-slate-800 bg-slate-950/50",
                )}
              >
                <dt
                  className={cn(
                    "flex flex-wrap items-center gap-1.5 text-xs",
                    target.primary ? "text-amber-200" : "text-slate-500",
                  )}
                >
                  {target.label}
                  {target.primary ? (
                    <span className="rounded bg-amber-300/15 px-1.5 py-0.5 text-[10px] font-semibold">
                      主参照
                    </span>
                  ) : null}
                </dt>
                <dd className="mt-1 break-words text-sm font-bold tabular-nums text-slate-100">
                  {target.value || "数値は未確認"}
                </dd>
                {target.note?.trim() ? (
                  <dd className="mt-2 whitespace-pre-wrap break-words text-xs leading-5 text-slate-300">
                    {target.note}
                  </dd>
                ) : null}
              </div>
            ))}
          </dl>
          {Boolean(member.targetStats?.length) && !member.targetStats?.some((target) => target.note?.trim()) ? (
            <p className="mt-3 text-xs leading-5 text-slate-400">
              この結果には目標値の個別の注意点がありません。元素共鳴や固有天賦などの影響は再調査で確認してください。
            </p>
          ) : null}
        </div>
      </div>
    </article>
  );
}

function ResultView({
  record,
  tab,
  onTabChange,
  onRevise,
  demo,
  conversation,
  onRename,
  disabled,
  reviseButtonRef,
}: {
  conversation: ResearchConversation | null;
  record: ResearchedTeamRecord;
  tab: ResultTab;
  onTabChange: (tab: ResultTab) => void;
  onRevise: () => void;
  demo: boolean;
  onRename: (teamId: string, title: string) => Promise<void>;
  disabled: boolean;
  reviseButtonRef: RefObject<HTMLButtonElement | null>;
}) {
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleInput, setTitleInput] = useState(record.title);
  const [titleError, setTitleError] = useState("");
  const [savingTitle, setSavingTitle] = useState(false);
  const editTitleButtonRef = useRef<HTMLButtonElement>(null);
  const restoreTitleFocus = useRef(false);
  useEffect(() => {
    if (!editingTitle && restoreTitleFocus.current) {
      restoreTitleFocus.current = false;
      editTitleButtonRef.current?.focus();
    }
  }, [editingTitle]);
  const saveTitle = async (event: FormEvent) => {
    event.preventDefault();
    const title = titleInput.trim();
    if (!title || Array.from(title).length > 80) {
      setTitleError("編成名は1〜80文字で入力してください。");
      return;
    }
    setSavingTitle(true);
    setTitleError("");
    try {
      await onRename(record.teamId, title);
      restoreTitleFocus.current = true;
      setEditingTitle(false);
    } catch (error) {
      setTitleError(error instanceof Error ? error.message : "編成名を変更できませんでした。");
    } finally {
      setSavingTitle(false);
    }
  };
  return (
    <section className="mx-auto w-full max-w-6xl flex-1 px-5 py-7 sm:px-8">
      <div className="flex flex-wrap items-start justify-between gap-5">
        <div className="min-w-0">
          <p className="flex items-center gap-2 text-xs font-semibold text-emerald-300">
            <ShieldCheck size={16} aria-hidden="true" />
            {demo
              ? "デモの調査結果・端末への保存なし"
              : "調査済み・この端末に保存"}
          </p>
          <div className="mt-2 flex flex-wrap items-center gap-3">
            <h1 className="min-w-0 break-words text-3xl font-bold text-slate-50">{record.title}</h1>
            {!editingTitle ? (
              <button
                ref={editTitleButtonRef}
                type="button"
                disabled={disabled}
                className="inline-flex min-h-11 items-center gap-2 rounded-lg border border-slate-600 px-3 text-sm text-slate-200 hover:bg-slate-800 disabled:opacity-50"
                onClick={() => {
                  setTitleInput(record.title);
                  setTitleError("");
                  setEditingTitle(true);
                }}
              >
                <Pencil size={15} aria-hidden="true" />編成名を編集
              </button>
            ) : null}
          </div>
          {editingTitle ? (
            <form className="mt-4 max-w-xl" onSubmit={(event) => void saveTitle(event)}>
              <label htmlFor="saved-team-title" className="mb-2 block text-sm font-medium text-slate-300">保存済みの編成名</label>
              <input
                id="saved-team-title"
                autoFocus
                type="text"
                value={titleInput}
                maxLength={80}
                disabled={savingTitle}
                aria-invalid={Boolean(titleError)}
                aria-describedby={titleError ? "saved-team-title-error" : undefined}
                onChange={(event) => setTitleInput(event.target.value)}
                className="min-h-11 w-full rounded-lg border border-slate-600 bg-slate-950 px-3 text-sm text-slate-100 focus-visible:outline-2 focus-visible:outline-amber-300 disabled:opacity-50"
              />
              {titleError ? <p id="saved-team-title-error" role="alert" className="mt-2 text-sm text-rose-300">{titleError}</p> : null}
              <div className="mt-3 flex flex-wrap gap-2">
                <button type="submit" disabled={savingTitle || !titleInput.trim()} className="min-h-11 rounded-lg bg-amber-300 px-4 font-semibold text-slate-950 hover:bg-amber-200 disabled:opacity-50">{savingTitle ? "保存中…" : "保存"}</button>
                <button type="button" disabled={savingTitle} className="min-h-11 rounded-lg border border-slate-600 px-4 text-slate-200 hover:bg-slate-800 disabled:opacity-50" onClick={() => { restoreTitleFocus.current = true; setEditingTitle(false); }}>キャンセル</button>
              </div>
            </form>
          ) : null}
          {record.gameVersion ? (
            <p className="mt-3 text-sm text-slate-400">
              対象バージョン：{record.gameVersion}
            </p>
          ) : null}
          {record.warnings.map((warning, index) => (
            <p
              key={index}
              className="mt-3 max-w-3xl whitespace-pre-wrap break-words text-sm leading-7 text-amber-200"
            >
              {warning}
            </p>
          ))}
        </div>
        <button ref={reviseButtonRef} className={actionClass} type="button" disabled={disabled} onClick={onRevise}>
          <RefreshCw size={16} aria-hidden="true" />
          条件を変えて再調査
        </button>
      </div>
      <div
        className="mt-6 flex gap-2 border-b border-slate-800"
        aria-label="結果の表示切り替え"
      >
        {(["build", "conversation"] as const).map((item) => (
          <button
            key={item}
            type="button"
            aria-pressed={tab === item}
            className={cn(
              "min-h-11 border-b-2 px-4 text-sm font-semibold",
              tab === item
                ? "border-amber-300 text-amber-300"
                : "border-transparent text-slate-400",
            )}
            onClick={() => onTabChange(item)}
          >
            {item === "build" ? "ビルド" : "会話"}
          </button>
        ))}
      </div>
      {record.teamReasoning ? <p className="mt-6 whitespace-pre-wrap break-words rounded-xl border border-slate-700 p-4 text-sm leading-6 text-slate-300">{record.teamReasoning}</p> : null}
      {tab === "build" ? (
        <div className="mt-6 grid gap-4 lg:grid-cols-2">
          {record.members.map((member, index) => (
            <BuildCard
              key={`${member.id}-${index}`}
              member={member}
              fixed={record.inputMembers?.find(input => input.name === member.name)}
              index={index}
            />
          ))}
        </div>
      ) : (
        <div className="mt-6">
          {conversation?.messages.length ? (
            <Conversation conversation={conversation} />
          ) : (
            <p className="text-sm leading-7 text-slate-400">
              この画面では過去の会話を読み込めません。条件の変更は「条件を変えて再調査」から選べます。
            </p>
          )}
        </div>
      )}
      {record.sources?.length ? (
        <details className="mt-6 rounded-xl border border-slate-800 p-4">
          <summary className="cursor-pointer text-sm text-slate-300">
            確認した出典（{record.sources.length}件）
          </summary>
          <ul className="mt-3 space-y-3 text-sm">
            {record.sources.map((source, index) => (
              <li key={index} className="break-words">
                {/^https?:\/\//i.test(source.url) ? (
                  <a
                    className="text-amber-200 underline"
                    href={source.url}
                    target="_blank"
                    rel="noreferrer"
                  >
                    {source.title || source.url}
                  </a>
                ) : (
                  source.title
                )}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </section>
  );
}

function Composer({
  value,
  game,
  onChange,
  onSubmit,
  disabled,
}: {
  value: string;
  game: GameId;
  onChange: (value: string) => void;
  onSubmit: () => void;
  disabled: boolean;
}) {
  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    onSubmit();
  };

  return (
    <div className="border-t border-slate-800 bg-slate-950/95 px-4 py-4 sm:px-8">
      <form
        className="mx-auto flex max-w-4xl items-end gap-3 rounded-2xl border border-slate-700 bg-slate-900 p-2 focus-within:border-amber-300/70"
        onSubmit={handleSubmit}
      >
        <label htmlFor="team-request" className="sr-only">
          調べたい編成
        </label>
        <textarea
          id="team-request"
          rows={2}
          className="max-h-36 min-h-12 min-w-0 flex-1 resize-none bg-transparent px-3 py-2 text-sm leading-6 text-slate-100 placeholder:text-slate-600 focus:outline-none disabled:cursor-not-allowed"
          placeholder={game === "genshin" ? "例：アルレッキーノ、夜蘭、ベネット、鍾離を調べたい" : "例：ホタル、ルアン・メェイ、開拓者・調和、ギャラガーを調べたい"}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={(event) => {
            if (
              event.key !== "Enter" ||
              event.shiftKey ||
              event.nativeEvent.isComposing ||
              event.nativeEvent.keyCode === 229
            ) return;
            event.preventDefault();
            if (!event.repeat && !disabled && value.trim()) {
              event.currentTarget.form?.requestSubmit();
            }
          }}
          aria-describedby="team-request-hint"
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
      <p id="team-request-hint" className="mx-auto mt-2 max-w-4xl text-center text-xs text-slate-600">
        Enterで送信・Shift+Enterで改行。調査結果はCodexの回答を検証してから保存します
      </p>
    </div>
  );
}

export function ResearchChatPrototype({
  repository = researchRepository,
}: {
  repository?: ResearchRepository;
}) {
  const [game, setGame] = useState<GameId>("genshin");
  const genshinChat = useResearchChat(repository, "genshin", game === "genshin");
  const starRailChat = useResearchChat(repository, "star_rail", game === "star_rail");
  const chat = game === "genshin" ? genshinChat : starRailChat;
  const [composers, setComposers] = useState<Record<GameId, string>>({ genshin: "", star_rail: "" });
  const composer = composers[game];
  const setComposer = (value: string) => setComposers(previous => ({ ...previous, [game]: value }));
  const [sidebarOpen, setSidebarOpen] = useState(() =>
    typeof window.matchMedia === "function"
      ? window.matchMedia("(min-width: 1024px)").matches
      : true,
  );
  const [resultTab, setResultTab] = useState<ResultTab>("build");
  const [revisions, setRevisions] = useState<Record<GameId, boolean>>({ genshin: false, star_rail: false });
  const revising = revisions[game];
  const setRevising = (value: boolean) => setRevisions(previous => ({ ...previous, [game]: value }));
  const [conditionDrafts, setConditionDrafts] = useState<Record<string, { members: import("./types").ResearchMemberInput[]; title: string }>>({});
  const draftKey = chat.conversation ? `${game}:${chat.conversation.sessionId}:${chat.conversation.updatedAt}` : "";
  const reviseButtonRef = useRef<HTMLButtonElement>(null);
  const restoreRevisionFocus = useRef(false);
  const composerRef = useRef<HTMLDivElement>(null);
  const pendingComposerFocus = useRef<{ source: Element | null } | null>({ source: null });
  const [composerFocusRequest, setComposerFocusRequest] = useState(0);
  const sidebarRef = useRef<HTMLElement>(null);
  const sidebarTriggerRef = useRef<HTMLButtonElement>(null);
  const wide = useSyncExternalStore(
    subscribeViewport,
    wideViewport,
    () => true,
  );
  const restoreSidebarFocus = useCallback(
    () => {
      if (
        document.activeElement === document.body ||
        sidebarRef.current?.contains(document.activeElement)
      ) sidebarTriggerRef.current?.focus();
    },
    [],
  );
  useEffect(() => {
    if (!sidebarOpen || wide) return;
    sidebarRef.current?.querySelector<HTMLButtonElement>("button")?.focus();
    return () => {
      queueMicrotask(restoreSidebarFocus);
    };
  }, [sidebarOpen, wide, restoreSidebarFocus]);
  const demo = repository.mode === "demo";
  const closeMobileSidebar = () => {
    if (window.matchMedia?.("(max-width: 1023px)").matches)
      setSidebarOpen(false);
  };
  const focusComposer = () =>
    composerRef.current?.querySelector("textarea")?.focus();
  const requestComposerFocus = () => {
    pendingComposerFocus.current = { source: document.activeElement };
    setComposerFocusRequest((request) => request + 1);
  };
  useEffect(() => {
    if (chat.busy || (sidebarOpen && !wide)) return;
    const pending = pendingComposerFocus.current;
    if (!pending) return;
    pendingComposerFocus.current = null;
    const active = document.activeElement;
    // 無効状態の解除後に戻す。待機中に選ばれた別の操作先からは奪わない。
    if (
      active === document.body ||
      active === pending.source ||
      composerRef.current?.contains(active)
    ) composerRef.current?.querySelector("textarea")?.focus();
  }, [composerFocusRequest, chat.busy, sidebarOpen, wide]);
  useEffect(() => {
    if (!revising && restoreRevisionFocus.current) {
      restoreRevisionFocus.current = false;
      reviseButtonRef.current?.focus();
    }
  }, [revising]);
  const handleSubmit = async () => {
    const message = composer.trim();
    if (!message || chat.busy) return;
    requestComposerFocus();
    if (await chat.send(message)) {
      setComposer("");
      setResultTab("build");
    }
  };
  return (
    <div className="flex min-h-dvh bg-slate-950 text-slate-100">
      {sidebarOpen ? (
        <>
          <button
            type="button"
            className="fixed inset-0 z-20 bg-black/60 lg:hidden"
            aria-label="サイドバーの背景を閉じる"
            onClick={() => setSidebarOpen(false)}
          />
          <aside
            ref={sidebarRef}
            className="fixed inset-y-0 left-0 z-30 flex w-[19rem] max-w-[85vw] flex-col border-r border-slate-800 bg-slate-950 p-4 lg:sticky lg:top-0 lg:h-dvh lg:shrink-0"
            aria-label="編成ナビゲーション"
            onKeyDown={(event) => {
              if (event.key === "Escape") setSidebarOpen(false);
              if (wide || event.key !== "Tab") return;
              const buttons =
                sidebarRef.current?.querySelectorAll<HTMLButtonElement>(
                  "button:not(:disabled)",
                );
              if (!buttons?.length) return;
              const first = buttons[0];
              const last = buttons[buttons.length - 1];
              if (event.shiftKey && document.activeElement === first) {
                event.preventDefault();
                last.focus();
              }
              if (!event.shiftKey && document.activeElement === last) {
                event.preventDefault();
                first.focus();
              }
            }}
          >
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="font-bold">ビルドレコメンダー</p>
                <p className="text-xs text-slate-500">Codexで調べて保存</p>
              </div>
              <button
                className="grid size-11 place-items-center rounded-lg hover:bg-slate-800"
                type="button"
                aria-label="サイドバーを閉じる"
                onClick={() => setSidebarOpen(false)}
              >
                <PanelLeftClose size={18} />
              </button>
            </div>
            <button
              type="button"
              disabled={chat.busy}
              className={cn(actionClass, "mt-6")}
              onClick={() => {
                chat.reset();
                setRevising(false);
                setComposer("");
                setResultTab("build");
                closeMobileSidebar();
                requestComposerFocus();
              }}
            >
              <Plus size={18} aria-hidden="true" />
              新しい編成を調べる
            </button>
            <div className="mt-7 flex items-center justify-between">
              <h2 className="text-xs font-semibold text-slate-400">
                保存した編成
              </h2>
              <button
                type="button"
                disabled={chat.listLoading}
                aria-label="保存一覧を更新"
                className="grid size-10 place-items-center rounded-lg hover:bg-slate-800 disabled:opacity-50"
                onClick={() => void chat.refreshTeams()}
              >
                <RefreshCw size={15} />
              </button>
            </div>
            <div className="mt-3 min-h-0 flex-1 space-y-2 overflow-y-auto">
              {chat.listLoading ? (
                <OperationProgress label="保存一覧を読み込み中…" className="text-slate-400" />
              ) : null}
              {chat.listError ? (
                <p
                  role="alert"
                  className="break-words text-sm leading-6 text-rose-300"
                >
                  保存一覧を取得できません。{chat.listError}
                </p>
              ) : null}
              {!chat.listLoading && !chat.listError && !chat.teams.length ? (
                <p className="rounded-xl border border-dashed border-slate-800 px-4 py-5 text-xs leading-6 text-slate-400">
                  調査が完了した編成がここに追加されます
                </p>
              ) : null}
              {chat.teams.map((team) => (
                <button
                  key={team.teamId}
                  type="button"
                  disabled={chat.busy}
                  aria-current={
                    chat.stage === "result" &&
                    chat.record?.teamId === team.teamId
                      ? "page"
                      : undefined
                  }
                  className="w-full rounded-xl border border-slate-800 px-3 py-3 text-left hover:bg-slate-800 disabled:opacity-50 aria-[current=page]:bg-slate-800"
                  onClick={() => {
                    void chat.openTeam(team.teamId);
                    setRevising(false);
                    setResultTab("build");
                    closeMobileSidebar();
                  }}
                >
                  <span className="block break-words text-sm font-semibold">
                    {team.title}
                  </span>
                  <span className="mt-1 block break-words text-xs leading-5 text-slate-400">
                    {team.memberNames.join(" / ")}
                  </span>
                </button>
              ))}
            </div>
            <p className="mt-4 flex items-center gap-2 border-t border-slate-800 pt-4 text-xs text-slate-500">
              <Database size={15} aria-hidden="true" />
              {demo
                ? "デモデータ・実際の調査は行いません"
                : "調査完了時にこの端末へ保存"}
            </p>
          </aside>
        </>
      ) : null}
      <main
        inert={sidebarOpen && !wide}
        className="flex min-h-dvh min-w-0 flex-1 flex-col"
      >
        <header className="flex min-h-16 flex-wrap items-center justify-between gap-3 border-b border-slate-800 px-4 sm:px-6">
          <div className="flex min-w-0 items-center gap-3">
            {!sidebarOpen ? (
              <button
                type="button"
                className="grid size-11 shrink-0 place-items-center rounded-lg hover:bg-slate-800"
                ref={sidebarTriggerRef}
                aria-label="サイドバーを開く"
                onClick={() => setSidebarOpen(true)}
              >
                <PanelLeftOpen size={19} />
              </button>
            ) : null}
            <p className="min-w-0 break-words text-sm font-semibold">
              {chat.stage === "result" ? chat.record?.title : "新しい編成調査"}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2" role="group" aria-label="ゲーム切替">
            {(["genshin", "star_rail"] as const).map(choice => <button type="button" key={choice}
              aria-pressed={game === choice} disabled={genshinChat.busy || starRailChat.busy}
              aria-describedby={genshinChat.busy || starRailChat.busy ? "game-switch-busy" : undefined}
              className={cn("min-h-11 rounded-lg border px-3 text-sm disabled:opacity-50", game === choice ? "border-amber-300 text-amber-200" : "border-slate-700 text-slate-300")}
              onClick={() => { setGame(choice); setResultTab("build"); }}>{choice === "genshin" ? "原神" : "崩壊：スターレイル"}</button>)}
            {genshinChat.busy || starRailChat.busy ? <span id="game-switch-busy" className="text-xs text-slate-400">処理中はゲームを切り替えられません</span> : null}
          </div>
          <div className="flex flex-wrap items-center justify-end gap-3">
            <span className="py-2 text-xs text-slate-400">
              {demo ? "デモ表示" : import.meta.env.DEV ? "開発版" : "正式版"}
            </span>
            <AppUpdateControl disabled={chat.busy} />
          </div>
        </header>
        <div className="flex min-h-0 flex-1 flex-col">
          {chat.error ? (
            <div
              role="alert"
              className="mx-5 mt-5 whitespace-pre-wrap break-words rounded-xl border border-rose-400/30 bg-rose-400/5 p-4 text-sm leading-6 text-rose-200"
            >
              {chat.error}
              <p className="mt-2">
                {revising
                  ? "条件を確認して、もう一度再調査してください。"
                  : "入力内容を確認して再送信するか、調査をやり直してください。"}
              </p>
            </div>
          ) : null}
          {chat.busy && chat.stage !== "researching" ? (
            <OperationProgress label={chat.activityLabel} className="px-5 pt-5" />
          ) : null}
          {!chat.conversation && chat.stage !== "result" ? (
            <EmptyConversation
              game={game}
              onUseExample={() => {
                setComposer(game === "genshin" ? EXAMPLE_PROMPT : "ホタル、ルアン・メェイ、開拓者・調和、ギャラガーの4人を調べたい。");
                focusComposer();
              }}
            />
          ) : null}
          {chat.stage === "researching" ? (
            <section className="mx-auto flex w-full max-w-3xl flex-1 flex-col justify-center px-5 py-12">
              <div className="rounded-2xl border border-slate-700 bg-slate-900 p-6">
                <ResearchProgressPanel progress={chat.progress} cancelling={chat.cancelling} />
                <p className="mt-5 text-xs leading-6 text-slate-400">
                  完了した編成だけ保存されます。
                </p>
                <button
                  type="button"
                  className={cn(actionClass, "mt-5")}
                  disabled={chat.cancelling}
                  onClick={() => void chat.cancel()}
                >
                  {chat.cancelling ? "キャンセル中…" : "調査をキャンセル"}
                </button>
              </div>
            </section>
          ) : null}
          {chat.conversation &&
          chat.stage !== "researching" &&
          chat.stage !== "result" &&
          !(revising && chat.record) ? (
            <section className="mx-auto w-full max-w-4xl flex-1 px-5 py-8 sm:px-8">
              <Conversation conversation={chat.conversation} />
              {chat.stage === "cancelled" ? (
                <p
                  role="status"
                  className="mt-5 rounded-xl border border-slate-700 p-4 text-sm text-slate-300"
                >
                  調査をキャンセルしました。条件を直すか、もう一度調査できます。
                </p>
              ) : null}
              {chat.conversation.members.length === 4 &&
              ["ready", "failed", "cancelled"].includes(
                chat.conversation.status,
              ) ? (
                <ResearchConditionsEditor
                  key={`${chat.conversation.sessionId}:${chat.conversation.updatedAt}`}
                  conversation={chat.conversation}
                  initialDraft={conditionDrafts[draftKey]}
                  onDraftChange={draft => setConditionDrafts(previous => ({ ...previous, [draftKey]: draft }))}
                  disabled={chat.busy}
                  onResearch={(members, title) => void chat.start(members, title)}
                />
              ) : ["ready", "failed", "cancelled"].includes(
                chat.conversation.status,
              ) ? (
                <button
                  type="button"
                  disabled={chat.busy}
                  className={cn(actionClass, "mt-6")}
                  onClick={() => void chat.start()}
                >
                  {chat.stage === "cancelled" || chat.stage === "error"
                    ? "もう一度調査する"
                    : "この内容で調査する"}
                  <ChevronRight size={17} aria-hidden="true" />
                </button>
              ) : (
                <p className="mt-5 text-sm text-slate-400">
                  不足している情報を下のチャットへ入力してください。
                </p>
              )}
            </section>
          ) : null}
          {revising && chat.record && chat.conversation &&
          (chat.stage === "result" || chat.stage === "error" || chat.stage === "cancelled") ? (
            <section className="mx-auto w-full max-w-5xl flex-1 px-5 py-7 sm:px-8">
              <button
                type="button"
                autoFocus
                disabled={chat.busy}
                className="min-h-11 rounded-lg border border-slate-600 px-4 text-sm text-slate-200 hover:bg-slate-800 disabled:opacity-50"
                onClick={() => {
                  chat.returnToResult();
                  restoreRevisionFocus.current = true;
                  setRevising(false);
                }}
              >
                結果に戻る
              </button>
              <h1 className="mt-5 text-2xl font-bold text-slate-50">条件を変えて再調査</h1>
              <ResearchConditionsEditor
                key={`${chat.conversation.sessionId}:${chat.record.teamId}`}
                conversation={chat.conversation}
                initialDraft={conditionDrafts[draftKey]}
                onDraftChange={draft => setConditionDrafts(previous => ({ ...previous, [draftKey]: draft }))}
                disabled={chat.busy}
                revision
                onResearch={async (members, title) => {
                  if (await chat.start(members, title)) {
                    setConditionDrafts(previous => { const next = { ...previous }; delete next[draftKey]; return next; });
                    setRevising(false);
                    setResultTab("build");
                  }
                }}
              />
            </section>
          ) : null}
          {chat.stage === "result" && chat.record && !revising ? (
            <ResultView
              key={chat.record.teamId}
              conversation={chat.conversation}
              record={chat.record}
              tab={resultTab}
              onTabChange={setResultTab}
              demo={demo}
              onRename={chat.renameTeam}
              disabled={chat.busy}
              reviseButtonRef={reviseButtonRef}
              onRevise={() => {
                setRevising(true);
              }}
            />
          ) : null}
        </div>
        {chat.stage !== "result" && !revising ? <div ref={composerRef}>
          <Composer
            game={game}
            value={composer}
            onChange={setComposer}
            onSubmit={() => void handleSubmit()}
            disabled={chat.busy}
          />
        </div> : null}
      </main>
    </div>
  );
}

export default ResearchChatPrototype;
