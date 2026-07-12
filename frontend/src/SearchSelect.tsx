import { useId, useMemo, useRef, useState, type KeyboardEvent } from "react";

export type SearchSelectOption = { value: string; label: string; detail?: string };

export function SearchSelect({ value, options, onChange, placeholder = "Search…", ariaLabel }: {
  value: string; options: SearchSelectOption[]; onChange: (value: string) => void; placeholder?: string; ariaLabel: string;
}) {
  const listId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const selected = options.find((option) => option.value === value);
  const [query, setQuery] = useState(selected?.label ?? "");
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return options.filter((option) => !needle || `${option.label} ${option.detail ?? ""}`.toLowerCase().includes(needle)).slice(0, 40);
  }, [options, query]);
  function choose(option: SearchSelectOption) {
    setQuery(option.label);
    setOpen(false);
    onChange(option.value);
  }
  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") { event.preventDefault(); setOpen(true); setActiveIndex((index) => Math.min(index + 1, matches.length - 1)); }
    else if (event.key === "ArrowUp") { event.preventDefault(); setActiveIndex((index) => Math.max(index - 1, 0)); }
    else if (event.key === "Enter" && open && matches[activeIndex]) { event.preventDefault(); choose(matches[activeIndex]); }
    else if (event.key === "Escape") { setOpen(false); setQuery(selected?.label ?? ""); }
  }
  return <div className="search-select">
    <input ref={inputRef} role="combobox" aria-label={ariaLabel} aria-expanded={open} aria-controls={listId}
      aria-autocomplete="list" aria-activedescendant={open && matches[activeIndex] ? `${listId}-${activeIndex}` : undefined}
      value={query} placeholder={placeholder} autoComplete="off" onKeyDown={onKeyDown}
      onFocus={(event) => { event.currentTarget.select(); setOpen(true); setActiveIndex(0); }}
      onChange={(event) => { setQuery(event.target.value); setOpen(true); setActiveIndex(0); }}
      onBlur={() => window.setTimeout(() => { setOpen(false); setQuery(options.find((option) => option.value === value)?.label ?? ""); }, 120)} />
    <button type="button" tabIndex={-1} aria-label="Show options" onMouseDown={(event) => event.preventDefault()} onClick={() => { setOpen((shown) => !shown); inputRef.current?.focus(); }}>⌄</button>
    {open && <div id={listId} role="listbox" className="search-select-options">
      {matches.length === 0 ? <p>No matching investments</p> : matches.map((option, index) => <button type="button" role="option" id={`${listId}-${index}`} aria-selected={option.value === value} className={index === activeIndex ? "active" : ""} key={`${option.value}-${option.detail ?? index}`} onMouseDown={(event) => event.preventDefault()} onMouseEnter={() => setActiveIndex(index)} onClick={() => choose(option)}><strong>{option.label}</strong>{option.detail && <small>{option.detail}</small>}</button>)}
    </div>}
  </div>;
}
