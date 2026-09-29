import { useId, type KeyboardEvent } from "react";

export interface PgTabOption<T extends string> {
  value: T;
  label: string;
}

/**
 * A row of pills picking one value -- radio semantics (not tabs: nothing
 * here switches a tab panel), with roving focus so arrow keys move the
 * selection the way native radios do.
 */
export function PgTabs<T extends string>({
  label,
  options,
  value,
  onChange,
  hint,
}: {
  label: string;
  options: ReadonlyArray<PgTabOption<T>>;
  value: T;
  onChange: (v: T) => void;
  /** One short line under the pills, for a control whose meaning isn't obvious from its label. */
  hint?: string;
}) {
  const id = useId();

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const idx = options.findIndex((o) => o.value === value);
    if (idx < 0) return;
    let next = -1;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = (idx + 1) % options.length;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") next = (idx - 1 + options.length) % options.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = options.length - 1;
    if (next < 0) return;
    e.preventDefault();
    onChange(options[next].value);
    e.currentTarget.querySelectorAll<HTMLButtonElement>('[role="radio"]')[next]?.focus();
  };

  return (
    <div className="pg-field">
      <span className="pg-label" id={id}>
        <span>{label}</span>
      </span>
      <div className="pg-tabs" role="radiogroup" aria-labelledby={id} onKeyDown={onKeyDown}>
        {options.map((opt) => {
          const active = opt.value === value;
          return (
            <button
              key={opt.value}
              type="button"
              role="radio"
              aria-checked={active}
              tabIndex={active ? 0 : -1}
              className={"pg-tab" + (active ? " pg-tab-active" : "")}
              onClick={() => onChange(opt.value)}
            >
              {opt.label}
            </button>
          );
        })}
      </div>
      {hint && <span className="pg-hint">{hint}</span>}
    </div>
  );
}
