import { useEffect, useId, useMemo, useState } from "react";

export type SearchSelectOption = { value: string; label: string; detail?: string };

export function SearchSelect({ value, options, onChange, placeholder = "Search…", ariaLabel }: {
  value: string; options: SearchSelectOption[]; onChange: (value: string) => void; placeholder?: string; ariaLabel: string;
}) {
  const listId = useId();
  const labels = useMemo(() => new Map(options.map((option) => [`${option.label}${option.detail ? ` · ${option.detail}` : ""}`, option.value])), [options]);
  const selected = options.find((option) => option.value === value);
  const selectedLabel = selected ? `${selected.label}${selected.detail ? ` · ${selected.detail}` : ""}` : "";
  const [query, setQuery] = useState(selectedLabel);
  useEffect(() => setQuery(selectedLabel), [selectedLabel]);
  return <div className="search-select">
    <input aria-label={ariaLabel} list={listId} value={query === selectedLabel ? selectedLabel : query} placeholder={placeholder}
      onFocus={(event) => event.currentTarget.select()}
      onChange={(event) => { const next = event.target.value; setQuery(next); const match = labels.get(next); if (match) onChange(match); }}
      onBlur={() => setQuery(selectedLabel)} autoComplete="off" />
    <datalist id={listId}>{options.map((option) => <option key={option.value} value={`${option.label}${option.detail ? ` · ${option.detail}` : ""}`} />)}</datalist>
    <span aria-hidden="true">⌕</span>
  </div>;
}
