import { cx, h } from "../core/dom.js";
import { Card } from "./card.js";
import { Icon } from "./icon.js";
import type { IconName } from "./icons.js";

export interface MetricCardProps {
  icon: IconName;
  label: string;
  value: string;
  /** Smaller value type, for long values such as `R$ 318,6 milhões`. */
  compact?: boolean;
  /** Trend line, e.g. `+28,4%`. */
  delta?: string;
  caption?: string;
}

/** KPI card: icon, label, big serif value, optional trend and caption. */
export function MetricCard({ icon, label, value, compact, delta, caption }: MetricCardProps): HTMLElement {
  return Card(
    { class: "metric" },
    Icon({ name: icon, size: 22, strokeWidth: 1.7, class: "metric__icon" }),
    h("p", { class: "metric__label" }, label),
    h("p", { class: cx("metric__value", compact && "metric__value--compact") }, value),
    delta &&
      h("p", { class: "metric__delta" }, Icon({ name: "trend-up", size: 15, strokeWidth: 2.25 }), h("span", null, delta)),
    caption && h("p", { class: "metric__caption" }, caption),
  );
}
