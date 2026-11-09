import { h } from "../core/dom.js";
import { Card } from "./card.js";
/** Vertical bar chart with a y-axis, light grid lines and month-style labels. */
export function BarChart({ title, data, step = 20 }) {
    const peak = Math.max(0, ...data.map((d) => d.value));
    const max = Math.max(step, Math.ceil(peak / step) * step);
    const ticks = Array.from({ length: max / step + 1 }, (_, i) => i * step);
    const pct = (value) => `${(value / max) * 100}%`;
    const axis = h("div", { class: "bar-chart__axis", "aria-hidden": "true" }, ticks.map((tick) => h("span", { class: "bar-chart__tick", style: `bottom:${pct(tick)}` }, tick)));
    const plot = h("div", { class: "bar-chart__plot" }, ticks.map((tick) => h("span", { class: "bar-chart__grid", style: `bottom:${pct(tick)}`, "aria-hidden": "true" })), h("div", { class: "bar-chart__bars" }, data.map((d, i) => h("div", {
        class: "bar-chart__bar",
        style: `height:${pct(d.value)};animation-delay:${i * 35}ms`,
        title: `${d.label}: ${d.value}`,
        role: "img",
        "aria-label": `${d.label}: ${d.value}`,
    }))));
    const labels = h("div", { class: "bar-chart__labels", "aria-hidden": "true" }, data.map((d) => h("span", null, d.label)));
    return Card({ class: "chart-card", title, titleId: "chart-openings-title" }, h("div", { class: "bar-chart" }, axis, plot, labels));
}
//# sourceMappingURL=bar-chart.js.map