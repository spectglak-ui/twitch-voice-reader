import clsx from "clsx";

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  description?: string;
  disabled?: boolean;
}

/** Interrupteur binaire utilisé dans tous les écrans de paramètres/filtres. */
export function Toggle({ checked, onChange, label, description, disabled }: ToggleProps) {
  return (
    <label
      className={clsx(
        "flex items-center justify-between gap-4 py-2",
        disabled && "opacity-40 pointer-events-none",
      )}
    >
      {(label || description) && (
        <span className="flex flex-col">
          {label && <span className="text-sm text-ink">{label}</span>}
          {description && <span className="text-xs text-ink-faint">{description}</span>}
        </span>
      )}
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={clsx(
          "relative h-6 w-11 shrink-0 rounded-full transition-colors",
          checked ? "bg-signal" : "bg-base-700",
        )}
      >
        <span
          className={clsx(
            "absolute top-0.5 h-5 w-5 rounded-full bg-white transition-transform",
            checked ? "translate-x-[22px]" : "translate-x-0.5",
          )}
        />
      </button>
    </label>
  );
}
