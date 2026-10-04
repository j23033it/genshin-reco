import type { RelicInput, ResearchMemberInput, TunnelSelection } from "./types";
import type { StarRailCatalog } from "../../domain/catalogTypes";

const selectClass = "min-h-11 w-full min-w-0 rounded-lg border border-slate-600 bg-slate-900 px-3 text-sm text-slate-100 focus-visible:outline-2 focus-visible:outline-amber-300 disabled:opacity-50";

export function StarRailRelicsEditor({ member, catalog, disabled, onChange }: {
  member: ResearchMemberInput; catalog: StarRailCatalog | null; disabled: boolean;
  onChange: (relics: RelicInput) => void;
}) {
  const relics = member.relics ?? { tunnel: null, ornament: null };
  const tunnel = relics.tunnel;
  const prefix = `relic-${member.slotIndex}`;
  const setTunnel = (selection: TunnelSelection | null) => onChange({ ...relics, tunnel: selection });
  function setName(index: number, value: string) {
    if (tunnel?.kind === "four_piece") setTunnel({ kind: "four_piece", set: value });
    else if (tunnel?.kind === "two_plus_two") {
      const sets: [string, string] = [...tunnel.sets]; sets[index] = value;
      setTunnel({ kind: "two_plus_two", sets });
    }
  }
  function options(entries: { id: string; name: string }[], value: string) {
    return <>
      <option value="">指定なし（編成に合わせて提案）</option>
      {value && !entries.some(entry => entry.name === value) ? <option value={value}>{value}（未登録・要確認）</option> : null}
      {entries.map(entry => <option key={entry.id} value={entry.name}>{entry.name}</option>)}
    </>;
  }
  return <fieldset className="mt-5 space-y-3 border-t border-slate-800 pt-4" disabled={disabled}>
    <legend className="text-sm font-medium text-slate-300">遺物セット（任意）</legend>
    <label className="block text-sm text-slate-300" htmlFor={`${prefix}-mode`}>トンネル遺物</label>
    <select id={`${prefix}-mode`} className={selectClass} value={tunnel?.kind ?? "unspecified"} onChange={event => setTunnel(event.target.value === "four_piece" ? { kind: "four_piece", set: "" } : event.target.value === "two_plus_two" ? { kind: "two_plus_two", sets: ["", ""] } : null)}>
      <option value="unspecified">指定なし（編成に合わせて提案）</option>
      <option value="four_piece">4セット</option><option value="two_plus_two">2＋2セット</option>
    </select>
    {(tunnel?.kind === "four_piece" ? [tunnel.set] : tunnel?.sets ?? []).map((name, index) => <div key={index} className="min-w-0">
      <label className="mb-1 block text-sm text-slate-400" htmlFor={`${prefix}-set-${index}`}>{tunnel?.kind === "four_piece" ? "4セット名" : `2＋2セット名 ${index + 1}`}</label>
      <select id={`${prefix}-set-${index}`} className={selectClass} value={name} onChange={event => setName(index, event.target.value)}>
        {options(catalog?.tunnelRelics.filter(entry => !entry.legacyOnly) ?? [], name)}
      </select>
    </div>)}
    <label className="block text-sm text-slate-300" htmlFor={`${prefix}-ornament`}>オーナメント（2セット）</label>
    <select id={`${prefix}-ornament`} className={selectClass} value={relics.ornament ?? ""} onChange={event => onChange({ ...relics, ornament: event.target.value || null })}>
      {options(catalog?.ornaments.filter(entry => !entry.legacyOnly) ?? [], relics.ornament ?? "")}
    </select>
  </fieldset>;
}
