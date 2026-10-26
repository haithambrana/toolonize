/** Color tags for terminals and workspaces (fixed palette, see Rust COLORS). */
import { COLOR_TAGS, type ColorTag } from "./ipc";

export const COLOR_LABELS: Record<ColorTag, string> = {
  red: "Red",
  orange: "Orange",
  yellow: "Yellow",
  green: "Green",
  teal: "Teal",
  blue: "Blue",
  purple: "Purple",
  pink: "Pink",
};

export function TagDot({ color }: { color: ColorTag | null | undefined }) {
  if (!color) return null;
  return (
    <span className={`tag-dot tag-${color}`} role="img" aria-label={`${COLOR_LABELS[color]} tag`} />
  );
}

/** A row of swatches plus "none"; calls `onPick` with the chosen tag. */
export function ColorSwatches({
  value,
  onPick,
  label,
}: {
  value: ColorTag | null;
  onPick: (color: ColorTag | null) => void;
  label: string;
}) {
  return (
    <div className="swatches" role="radiogroup" aria-label={label}>
      <button
        type="button"
        role="radio"
        aria-checked={value === null}
        className="swatch swatch-none"
        title="No color"
        aria-label="No color"
        onClick={() => onPick(null)}
      />
      {COLOR_TAGS.map((c) => (
        <button
          key={c}
          type="button"
          role="radio"
          aria-checked={value === c}
          className={`swatch tag-${c}`}
          title={COLOR_LABELS[c]}
          aria-label={COLOR_LABELS[c]}
          onClick={() => onPick(c)}
        />
      ))}
    </div>
  );
}
