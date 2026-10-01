import { useEffect, useState } from "react";
import type { Catalog, StarRailCatalog } from "../../domain/catalogTypes";
import { loadCatalog, loadStarRailCatalog } from "../catalog/loadCatalog";
import { cn } from "../../lib/cn";
import { OperationProgress } from "../../components/OperationProgress";
import type { ResearchConversation, ResearchMemberInput } from "./types";

import { StarRailRelicsEditor } from "./StarRailRelicsEditor";

import { validateRelicInput } from "./validateRelics";

const CONSTELLATIONS = [null, 0, 1, 2, 3, 4, 5, 6] as const;

export function ResearchConditionsEditor({
  conversation,
  disabled,
  onResearch,
  revision = false,
  initialDraft,
  onDraftChange,
}: {
  conversation: ResearchConversation;
  disabled: boolean;
  onResearch: (members: ResearchMemberInput[], title: string | null) => void;
  revision?: boolean;
  initialDraft?: { members: ResearchMemberInput[]; title: string };
  onDraftChange?: (draft: { members: ResearchMemberInput[]; title: string }) => void;
}) {
  const starRail = conversation.game === "star_rail";
  const [title, setTitle] = useState(
    revision ? conversation.title ?? "" : initialDraft?.title ?? conversation.title ?? "",
  );
  const [members, setMembers] = useState<ResearchMemberInput[]>(() =>
    (initialDraft?.members ?? conversation.members).map((member) => ({ ...member })),
  );
  const [catalog, setCatalog] = useState<Pick<Catalog, "characters" | "weapons"> | null>(null);
  const [starRailCatalog, setStarRailCatalog] = useState<StarRailCatalog | null>(null);
  const [catalogFailed, setCatalogFailed] = useState(false);

  useEffect(() => {
    let active = true;
    const load = starRail ? loadStarRailCatalog().then(loaded => {
      if (active) setStarRailCatalog(loaded);
      return { characters: loaded.characters.map(character => ({ ...character, weaponType: character.path, imageUrl: character.imageUrl ?? "", rarity: 5 })), weapons: loaded.lightCones.map(cone => ({ ...cone, weaponType: cone.path, imageUrl: cone.imageUrl ?? "", rarity: 5 })) };
    }) : loadCatalog();
    void load.then(
      (loaded) => {
        if (active) setCatalog(loaded);
      },
      () => {
        if (active) setCatalogFailed(true);
      },
    );
  return () => {
      active = false;
    };
  }, [starRail]);

  const changeMember = (slotIndex: number, change: Partial<ResearchMemberInput>) => {
    const next = members.map(member => member.slotIndex === slotIndex ? { ...member, ...change } : member);
    setMembers(next);
    onDraftChange?.({ members: next, title });
  };

  const inputErrors = starRail ? members.map(member => validateRelicInput(member, starRailCatalog)).filter(Boolean) : [];

  return (
    <section className="mt-6 rounded-2xl border border-slate-700 bg-slate-900/70 p-4 sm:p-6" aria-labelledby="research-conditions-title">
      <h2 id="research-conditions-title" className="text-lg font-semibold text-slate-50">
        {revision ? starRail ? "光円錐・星魂・重畳・遺物を選び直す" : "武器と凸を選び直す" : "4人の条件を選ぶ"}
      </h2>
      <p className="mt-2 text-pretty text-sm leading-6 text-slate-400">
        {starRail ? revision
          ? "キャラクターはこの4人で固定します。光円錐・星魂・重畳・遺物を選んで再調査できます。指定したセットは維持し、指定していない部分を編成全体に合わせて提案します。"
          : "星魂・光円錐・重畳が未確定なら「指定なし」のままで大丈夫。調査時に一般的な前提を選びます。遺物は指定したセットを維持し、指定していない部分を編成全体に合わせて提案します。" : revision
          ? "キャラクターはこの4人で固定します。武器・命ノ星座・精錬を選んで再調査できます。"
          : "凸や武器が未確定なら「指定なし」のままで大丈夫。調査時に一般的な前提を選びます。"}
      </p>
      {!revision ? <div className="mt-5 max-w-xl">
        <label htmlFor="research-team-title" className="mb-2 block text-sm font-medium text-slate-300">
          編成名
        </label>
        <input
          id="research-team-title"
          type="text"
          value={title}
          maxLength={80}
          disabled={disabled}
          placeholder="空欄なら調査結果から自動で名前を付けます"
          onChange={(event) => { setTitle(event.target.value); onDraftChange?.({ members, title: event.target.value }); }}
          className="min-h-11 w-full rounded-lg border border-slate-600 bg-slate-900 px-3 text-sm text-slate-100 focus-visible:outline-2 focus-visible:outline-amber-300 disabled:opacity-50"
        />
        <p className="mt-2 text-sm text-slate-400">任意・80文字以内。保存後も変更できます。</p>
      </div> : null}
      {catalogFailed ? (
        <p role="status" className="mt-3 text-sm text-amber-200">
          {starRail ? "カタログを読み込めません。指定は保持しています。読み込みが復旧してから再確認してください。" : revision
            ? "武器一覧を読み込めません。現在の武器か「指定なし」を選べます。"
            : "武器一覧を読み込めないため、武器名は手入力できます。"}
        </p>
      ) : null}
      {!catalog && !catalogFailed ? (
        <OperationProgress className="mt-3" label="装備一覧を読み込み中…" />
      ) : null}
      <div className="mt-5 grid gap-4 xl:grid-cols-2">
        {members.map((member) => {
          const character = catalog?.characters.find(
            (entry) => entry.name === member.name,
          );
          const weapons = catalog?.weapons.filter(
            (weapon) => !character || weapon.weaponType === character.weaponType,
          );
          const selectedOutsideCatalog =
            member.weapon &&
            !weapons?.some((weapon) => weapon.name === member.weapon);
          const weaponId = `weapon-${member.slotIndex}`;
          const refinementId = `refinement-${member.slotIndex}`;
          return (
            <div key={member.slotIndex} role="group" aria-label={`${member.name}の条件`} className="min-w-0 rounded-xl border border-slate-700 bg-slate-950/60 p-4">
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <h3 className="break-words font-semibold text-slate-100">
                  {member.name}
                </h3>
                {character ? (
                  <span className="text-xs text-slate-400">{character.weaponType}</span>
                ) : null}
              </div>
              <fieldset className="mt-4" disabled={disabled}>
                <legend className="mb-2 text-sm font-medium text-slate-300">{starRail ? "星魂" : "命ノ星座"}</legend>
                <div className="flex flex-wrap gap-2">
                  {CONSTELLATIONS.map((value) => {
                    const label = value === null ? "指定なし" : `${value}凸`;
                    const selected = (member.constellation ?? null) === value;
                    return (
                      <button
                        key={label}
                        type="button"
                        aria-label={`${member.name}：${label}`}
                        aria-pressed={selected}
                        className={cn(
                          "min-h-10 rounded-lg border px-3 text-sm font-medium focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-amber-300 disabled:opacity-50",
                          selected
                            ? "border-amber-300 bg-amber-300 text-slate-950"
                            : "border-slate-600 text-slate-300 hover:border-slate-400 hover:bg-slate-800",
                        )}
                        onClick={() => changeMember(member.slotIndex, { constellation: value })}
                      >
                        {label}
                      </button>
                    );
                  })}
                </div>
              </fieldset>
              <div className="mt-4 grid gap-3 sm:grid-cols-[minmax(0,1fr)_8rem]">
                <div className="min-w-0">
                  <label htmlFor={weaponId} className="mb-2 block text-sm font-medium text-slate-300">
                    {starRail ? "光円錐" : "武器"}
                  </label>
                  {catalogFailed && !revision ? (
                    <input
                      id={weaponId}
                      type="text"
                      value={member.weapon ?? ""}
                      placeholder="空欄なら指定なし"
                      disabled={disabled}
                      onChange={(event) => changeMember(member.slotIndex, {
                        weapon: event.target.value || null,
                        refinement: null,
                      })}
                      className="min-h-11 w-full rounded-lg border border-slate-600 bg-slate-900 px-3 text-sm text-slate-100 focus-visible:outline-2 focus-visible:outline-amber-300 disabled:opacity-50"
                    />
                  ) : (
                    <select
                      id={weaponId}
                      value={member.weapon ?? ""}
                      disabled={disabled || (!catalog && !catalogFailed)}
                      onChange={(event) => changeMember(member.slotIndex, {
                        weapon: event.target.value || null,
                        refinement: null,
                      })}
                      className="min-h-11 w-full rounded-lg border border-slate-600 bg-slate-900 px-3 text-sm text-slate-100 focus-visible:outline-2 focus-visible:outline-amber-300 disabled:opacity-50"
                    >
                      <option value="">指定なし（未確定）</option>
                      {catalogFailed && member.weapon ? (
                        <option value={member.weapon}>{member.weapon}</option>
                      ) : null}
                      {!catalogFailed && selectedOutsideCatalog ? (
                        <option value={member.weapon ?? ""}>{member.weapon}</option>
                      ) : null}
                      {weapons?.map((weapon) => (
                        <option key={weapon.id} value={weapon.name}>{weapon.name}</option>
                      ))}
                    </select>
                  )}
                </div>
                <div>
                  <label htmlFor={refinementId} className="mb-2 block text-sm font-medium text-slate-300">
                    {starRail ? "重畳" : "精錬"}
                  </label>
                  <select
                    id={refinementId}
                    value={member.refinement ?? ""}
                    disabled={disabled || !member.weapon}
                    onChange={(event) => changeMember(member.slotIndex, {
                      refinement: event.target.value ? Number(event.target.value) : null,
                    })}
                    className="min-h-11 w-full rounded-lg border border-slate-600 bg-slate-900 px-3 text-sm text-slate-100 focus-visible:outline-2 focus-visible:outline-amber-300 disabled:opacity-50"
                  >
                    <option value="">指定なし</option>
                    {[1, 2, 3, 4, 5].map((value) => (
                      <option key={value} value={value}>{starRail ? "S" : "R"}{value}</option>
                    ))}
                  </select>
                </div>
              </div>
              {starRail ? <StarRailRelicsEditor member={member} catalog={starRailCatalog} disabled={disabled} onChange={relics => changeMember(member.slotIndex, { relics })} /> : null}
            </div>
          );
        })}
      </div>
      {inputErrors.length ? <p role="alert" className="mt-4 break-words text-sm text-amber-200">{[...new Set(inputErrors)].join("\n")}</p> : null}
      <button
        type="button"
        disabled={disabled || inputErrors.length > 0}
        className="mt-6 inline-flex min-h-11 items-center justify-center rounded-xl bg-amber-300 px-5 py-2 font-semibold text-slate-950 hover:bg-amber-200 disabled:cursor-not-allowed disabled:opacity-50"
        onClick={() => onResearch(members, title.trim() || null)}
      >
        {revision ? "この条件で再調査する" : "この条件で調査する"}
      </button>
    </section>
  );
}
