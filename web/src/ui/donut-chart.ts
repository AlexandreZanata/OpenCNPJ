import { h, s } from "../core/dom.js";
import { formatPercent } from "../core/format.js";
import { Card } from "./card.js";

export interface DonutSlice {
  id: string;
  label: string;
  /** Percent of the whole (0–100). */
  share: number;
}

export interface DonutChartProps {
  title: string;
  slices: readonly DonutSlice[];
  centerValue: string;
  centerLabel: string;
}

const RADIUS = 50;

/** Donut chart with a centered total and a legend with percentages. */
export function DonutChart({ title, slices, centerValue, centerLabel }: DonutChartProps): HTMLElement {
  let offset = 0;
  const arcs = slices.map((slice) => {
    const arc = s("circle", {
      class: `donut__slice donut__slice--${slice.id}`,
      cx: 60,
      cy: 60,
      r: RADIUS,
      fill: "none",
      "stroke-width": 20,
      pathLength: 100,
      "stroke-dasharray": `${slice.share} ${100 - slice.share}`,
      "stroke-dashoffset": -offset,
      transform: "rotate(-90 60 60)",
    });
    offset += slice.share;
    return arc;
  });

  const ring = h(
    "div",
    { class: "donut__ring" },
    s("svg", { viewBox: "0 0 120 120", role: "img", "aria-label": `${title}: ${slices.map((x) => `${x.label} ${formatPercent(x.share)}`).join(", ")}` }, arcs),
    h("div", { class: "donut__center" }, h("strong", null, centerValue), h("span", null, centerLabel)),
  );

  const legend = h(
    "ul",
    { class: "donut__legend" },
    slices.map((slice) =>
      h(
        "li",
        null,
        h("span", { class: `donut__swatch donut__swatch--${slice.id}`, "aria-hidden": "true" }),
        h("span", { class: "donut__name" }, slice.label),
        h("span", { class: "donut__share" }, formatPercent(slice.share)),
      ),
    ),
  );

  return Card({ class: "chart-card", title, titleId: "chart-porte-title" }, h("div", { class: "donut" }, ring, legend));
}
