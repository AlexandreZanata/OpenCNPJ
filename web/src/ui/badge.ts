import { h } from "../core/dom.js";

export type BadgeTone = "success" | "warning" | "danger" | "neutral";

export interface BadgeProps {
  label: string;
  tone?: BadgeTone;
}

/** Small rounded status label. */
export function Badge({ label, tone = "neutral" }: BadgeProps): HTMLElement {
  return h("span", { class: `badge badge--${tone}` }, label);
}
