interface SliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  onChange: (value: number) => void;
  formatValue?: (value: number) => string;
}

/** Curseur générique pour volume/vitesse/hauteur/débit, avec valeur formatée. */
export function Slider({ label, value, min, max, step = 0.01, unit, onChange, formatValue }: SliderProps) {
  const displayValue = formatValue ? formatValue(value) : `${value}${unit ?? ""}`;

  return (
    <div className="py-2">
      <div className="mb-2 flex items-center justify-between">
        <span className="field-label">{label}</span>
        <span className="font-mono text-xs text-ink-muted">{displayValue}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-base-700 accent-signal"
      />
    </div>
  );
}
