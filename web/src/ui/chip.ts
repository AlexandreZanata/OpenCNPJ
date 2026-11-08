import { h } from "../core/dom.js";

export interface ChipProps {
  label: string;
  onClick?: () => void;
}

/** Pill-shaped suggestion (query examples). */
export function Chip({ label, onClick }: ChipProps): HTMLButtonElement {
  return h("button", { class: "chip", type: "button", on: onClick ? { click: onClick } : undefined }, label);
}
