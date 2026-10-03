import { useRef, useState } from "react";

interface EquipmentOption {
  value: string;
  label: string;
}

const fieldClass = "min-h-11 w-full min-w-0 rounded-lg border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-slate-100 focus-visible:outline-2 focus-visible:outline-amber-300 disabled:opacity-50";

function normalizeSearch(value: string) {
  return value.normalize("NFKC").trim().toLocaleLowerCase("ja");
}

/** Callers supply only equipment compatible with the current character and game. */
export function EquipmentSelect({
  id, label, value, options, onChange, disabled = false, searchDisabled = false,
  emptyLabel = "指定なし（未確定）",
}: {
  id: string;
  label: string;
  value: string;
  options: readonly EquipmentOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  searchDisabled?: boolean;
  emptyLabel?: string;
}) {
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const composing = useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const normalizedQuery = normalizeSearch(query);
  const matching = options.filter(option => normalizeSearch(option.label).includes(normalizedQuery));
  const selected = options.find(option => option.value === value);
  const selectedOutsideSearch = selected && !matching.includes(selected);
  const unavailable = Boolean(value && !selected);
  const resultId = `${id}-search-result`;

  const clear = () => {
    setInput("");
    setQuery("");
    inputRef.current?.focus();
  };

  return (
    <div className="min-w-0">
      <label htmlFor={`${id}-search`} className="mb-2 block text-sm font-medium text-slate-300">
        {label}を検索
      </label>
      <div className="flex gap-2">
        <input
          ref={inputRef}
          id={`${id}-search`}
          type="search"
          value={input}
          placeholder={`${label}名で絞り込み`}
          disabled={disabled || searchDisabled}
          aria-controls={id}
          aria-describedby={resultId}
          className={fieldClass}
          onCompositionStart={() => { composing.current = true; }}
          onCompositionEnd={event => {
            composing.current = false;
            setInput(event.currentTarget.value);
            setQuery(event.currentTarget.value);
          }}
          onChange={event => {
            setInput(event.target.value);
            if (!composing.current) setQuery(event.target.value);
          }}
          onKeyDown={event => {
            if (event.key === "Escape" && !composing.current && !event.nativeEvent.isComposing && event.keyCode !== 229) {
              event.preventDefault();
              clear();
            }
          }}
        />
        <button
          type="button"
          aria-label={`${label}の検索をクリア`}
          disabled={disabled || searchDisabled || !input}
          onClick={clear}
          className="min-w-11 shrink-0 rounded-lg border border-slate-600 px-2 text-sm text-slate-300 hover:bg-slate-800 focus-visible:outline-2 focus-visible:outline-amber-300 disabled:opacity-50"
        >
          <span aria-hidden="true">×</span>
        </button>
      </div>
      <p id={resultId} role="status" className="mt-2 text-xs text-slate-400">
        {normalizedQuery ? matching.length ? `${matching.length}件が一致しました。` : `一致する${label}はありません。` : "一覧からも選べます。"}
        {selectedOutsideSearch ? " 選択中の装備は保持しています。" : ""}
      </p>
      <label htmlFor={id} className="mb-2 mt-3 block text-sm font-medium text-slate-300">{label}</label>
      <select
        id={id}
        value={value}
        disabled={disabled}
        aria-invalid={unavailable || undefined}
        aria-describedby={resultId}
        onChange={event => onChange(event.target.value)}
        className={fieldClass}
      >
        <option value="">{emptyLabel}</option>
        {unavailable ? <option value={value} disabled>装備を選び直してください</option> : null}
        {selectedOutsideSearch ? <option value={selected.value}>{selected.label}（選択中）</option> : null}
        {matching.map(option => <option key={option.value} value={option.value}>{option.label}</option>)}
      </select>
    </div>
  );
}
