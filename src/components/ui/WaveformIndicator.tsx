import clsx from "clsx";

/** Trois barres animées représentant une lecture vocale en cours. C'est
 * l'élément de signature visuelle du produit : il n'apparaît QUE lorsqu'une
 * lecture est réellement active (jamais en pur décor), dans la barre
 * supérieure et à côté de chaque chaîne connectée. */
export function WaveformIndicator({ active, className }: { active: boolean; className?: string }) {
  if (!active) return null;
  return (
    <span className={clsx("waveform text-live", className)} aria-label="Lecture vocale en cours">
      <span />
      <span />
      <span />
    </span>
  );
}
