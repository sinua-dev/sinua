import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { Icon } from "./Icon";

/**
 * Value slider: one 32px block where the fill *is* the value, the label
 * rides on the fill and the readout sits at the right (the pattern the
 * upstream thinking-orbs Studio ships). The real control is a native `<input type="range">`
 * laid invisibly on top -- it already implements the WAI-ARIA slider
 * keyboard contract (arrows, Home/End, PageUp/Down) and announces itself
 * to screen readers, so no slider library is needed.
 *
 * Two additions over upstream: clicking the readout turns it into a
 * numeric field (Enter commits, Esc cancels, blur commits), and a
 * `modified` knob shows an accent dot plus a reset button (double-clicking
 * the slider resets too).
 */
export function PgSlider({
  label,
  value,
  min,
  max,
  step,
  display,
  onChange,
  modified,
  onReset,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  display: string;
  onChange: (v: number) => void;
  /** True when the value differs from the preset default. */
  modified?: boolean;
  /** Restores the default; shown as a button while `modified`. */
  onReset?: () => void;
}) {
  const pct = max > min ? ((Math.min(max, Math.max(min, value)) - min) / (max - min)) * 100 : 0;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const numRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) numRef.current?.select();
  }, [editing]);

  // Snap a typed value onto the slider's own grid and clamp it, then
  // re-round so float dust never reaches the readout or the engine.
  const snap = (raw: number): number | null => {
    if (!Number.isFinite(raw)) return null;
    const stepped = step > 0 ? Math.round((raw - min) / step) * step + min : raw;
    const clamped = Math.min(max, Math.max(min, stepped));
    return Math.round(clamped * 1000) / 1000;
  };

  const commit = () => {
    const v = snap(parseFloat(draft));
    setEditing(false);
    if (v != null && v !== value) onChange(v);
  };

  const onNumKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      setEditing(false);
    }
  };

  const canReset = Boolean(modified && onReset);

  return (
    <div className="pg-field pg-slider-field">
      <div className="pg-vslider" style={{ "--pct": `${pct}%` } as CSSProperties}>
        <div className="pg-vslider-fill" aria-hidden="true" />
        <span className="pg-vslider-label">
          {label}
          {modified && <i className="pg-dot" title="Changed from the preset default" />}
        </span>
        <span className="pg-vslider-value">
          {editing ? (
            <input
              ref={numRef}
              className="pg-vslider-numinput"
              type="number"
              inputMode="decimal"
              min={min}
              max={max}
              step={step}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={onNumKey}
              onBlur={commit}
              aria-label={`${label}, typed value`}
            />
          ) : (
            <button
              type="button"
              className="pg-vslider-valuebtn"
              title="Click to type a value"
              onClick={() => {
                setDraft(String(value));
                setEditing(true);
              }}
            >
              {display}
            </button>
          )}
        </span>
        <input
          type="range"
          className="pg-vslider-input"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          onDoubleClick={() => onReset?.()}
          aria-label={label}
          aria-valuetext={display}
        />
      </div>
      <button
        type="button"
        className="icon-btn pg-vslider-reset"
        data-visible={canReset}
        tabIndex={canReset ? 0 : -1}
        aria-hidden={!canReset}
        aria-label={`Reset ${label} to its default`}
        title="Reset to default"
        onClick={onReset}
      >
        <Icon name="reset" />
      </button>
    </div>
  );
}
