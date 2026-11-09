import { h, s } from "../../core/dom.js";
import type { ChartPoint } from "./types.js";

const decimal = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 });
const compact = new Intl.NumberFormat("pt-BR", { notation: "compact", maximumFractionDigits: 1 });

export function formatValue(value: number): string {
  return decimal.format(value);
}

/** "janeiro de 2025" -> "jan/25"; "03/2025" -> "mar/25"; other labels pass through. */
export function shortMonth(label: string): string {
  const months = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
  const named = /^(\p{L}+) de (\d{4})$/u.exec(label);
  if (named) {
    const index = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"].indexOf(named[1]!.toLowerCase());
    if (index >= 0) return `${months[index]}/${named[2]!.slice(2)}`;
  }
  const numeric = /^(\d{2})\/(\d{4})$/.exec(label);
  if (numeric) return `${months[Number(numeric[1]) - 1] ?? numeric[1]}/${numeric[2]!.slice(2)}`;
  return label;
}

function niceStep(max: number, ticks: number): number {
  const raw = Math.max(max, 1) / ticks;
  const power = 10 ** Math.floor(Math.log10(raw));
  const unit = [1, 2, 2.5, 5, 10].find((m) => m * power >= raw) ?? 10;
  return unit * power;
}

export interface Series {
  name: string;
  points: readonly ChartPoint[];
}

/** Categorical slots validated together (adjacent CVD ΔE ≥ 8); one series uses the brand green. */
const SERIES_CLASSES = ["viz-series-1", "viz-series-2", "viz-series-3"] as const;

function dataTable(head: string, series: readonly Series[]): HTMLElement {
  const labels = series[0]?.points.map((p) => p.label) ?? [];
  return h(
    "div",
    { class: "chat-table-wrap" },
    h(
      "table",
      { class: "chat-table" },
      h("thead", null, h("tr", null, h("th", { scope: "col" }, head), series.map((line) => h("th", { scope: "col", class: "num" }, line.name)))),
      h(
        "tbody",
        null,
        labels.map((label, i) =>
          h("tr", null, h("th", { scope: "row" }, label), series.map((line) => h("td", { class: "num" }, formatValue(line.points[i]?.value ?? 0)))),
        ),
      ),
    ),
  );
}

/** Line chart over months: 2px lines, ringed end dots, crosshair tooltip and a table view. */
export function LineChart(title: string, series: readonly Series[]): HTMLElement {
  const width = 720;
  const height = 240;
  const pad = { top: 16, right: 56, bottom: 30, left: 48 };
  const count = Math.max(...series.map((line) => line.points.length), 1);
  const peak = Math.max(0, ...series.flatMap((line) => line.points.map((p) => p.value)));
  const step = niceStep(peak, 4);
  const max = Math.max(step, Math.ceil(peak / step) * step);
  const x = (i: number): number => pad.left + (count === 1 ? (width - pad.left - pad.right) / 2 : (i * (width - pad.left - pad.right)) / (count - 1));
  const y = (v: number): number => pad.top + (1 - v / max) * (height - pad.top - pad.bottom);
  const labels = series[0]?.points.map((p) => shortMonth(p.label)) ?? [];
  const every = Math.max(1, Math.ceil(count / 8));
  const single = series.length === 1;
  const seriesClass = (i: number): string => (single ? "viz-series-brand" : SERIES_CLASSES[i] ?? "viz-series-3");

  const grid = Array.from({ length: Math.round(max / step) + 1 }, (_, i) => i * step).map((tick) => [
    s("line", { class: "viz-grid", x1: pad.left, x2: width - pad.right, y1: y(tick), y2: y(tick) }),
    s("text", { class: "viz-tick", x: pad.left - 8, y: y(tick) + 4, "text-anchor": "end" }),
  ]);
  grid.forEach(([, text], i) => {
    text!.textContent = compact.format(i * step);
  });
  const xLabels = labels.map((label, i) => {
    if (i % every !== 0 && i !== count - 1) return null;
    const text = s("text", { class: "viz-tick", x: x(i), y: height - 8, "text-anchor": "middle" });
    text.textContent = label;
    return text;
  });
  // End labels sit beside the last point and are pushed apart when the series end close.
  const ends = series
    .map((line, si) => ({ si, last: line.points[line.points.length - 1] }))
    .filter((end): end is { si: number; last: ChartPoint } => end.last !== undefined)
    .map((end) => ({ ...end, at: y(end.last.value) + 4 }))
    .sort((a, b) => a.at - b.at);
  ends.forEach((end, i) => {
    const previous = ends[i - 1];
    if (previous && end.at - previous.at < 13) end.at = previous.at + 13;
  });
  const lines = series.map((line, si) => {
    const path = line.points.map((p, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(p.value).toFixed(1)}`).join("");
    const last = line.points[line.points.length - 1];
    const end = last ? s("circle", { class: `viz-dot ${seriesClass(si)}`, cx: x(line.points.length - 1), cy: y(last.value), r: 4 }) : null;
    const label = ends.find((e) => e.si === si);
    const value = last && label ? s("text", { class: "viz-end", x: x(line.points.length - 1) + 9, y: label.at }) : null;
    if (value && last) value.textContent = formatValue(last.value);
    return [s("path", { class: `viz-line ${seriesClass(si)}`, d: path }), end, value];
  });
  const cross = s("line", { class: "viz-cross", y1: pad.top, y2: height - pad.bottom, x1: 0, x2: 0, visibility: "hidden" });
  const hoverDots = series.map((_, si) => s("circle", { class: `viz-dot ${seriesClass(si)}`, r: 4, cx: 0, cy: 0, visibility: "hidden" }));
  const overlay = s("rect", { class: "viz-overlay", x: pad.left, y: pad.top, width: width - pad.left - pad.right, height: height - pad.top - pad.bottom });
  const svg = s(
    "svg",
    { class: "viz-svg", viewBox: `0 0 ${width} ${height}`, role: "img", "aria-label": title },
    grid.flat(),
    xLabels.filter((t): t is SVGTextElement => t !== null),
    lines.flat(),
    cross,
    hoverDots,
    overlay,
  );
  const tooltip = h("div", { class: "viz-tooltip", hidden: true, role: "status" });
  const plot = h("div", { class: "viz-plot" }, svg, tooltip);

  const show = (event: PointerEvent): void => {
    const box = svg.getBoundingClientRect();
    const px = ((event.clientX - box.left) / box.width) * width;
    const i = Math.max(0, Math.min(count - 1, Math.round(((px - pad.left) / (width - pad.left - pad.right)) * (count - 1))));
    cross.setAttribute("x1", String(x(i)));
    cross.setAttribute("x2", String(x(i)));
    cross.setAttribute("visibility", "visible");
    series.forEach((line, si) => {
      const point = line.points[i];
      const dot = hoverDots[si]!;
      if (!point) return dot.setAttribute("visibility", "hidden");
      dot.setAttribute("cx", String(x(i)));
      dot.setAttribute("cy", String(y(point.value)));
      dot.setAttribute("visibility", "visible");
    });
    tooltip.replaceChildren(
      h("strong", null, series[0]?.points[i]?.label ?? ""),
      ...series.map((line, si) => h("span", { class: "viz-tooltip__row" }, h("i", { class: `viz-key ${seriesClass(si)}` }), `${line.name}: `, h("b", null, formatValue(line.points[i]?.value ?? 0)))),
    );
    tooltip.hidden = false;
    const left = (x(i) / width) * box.width;
    tooltip.style.left = `${Math.min(Math.max(left, 70), box.width - 70)}px`;
  };
  overlay.addEventListener("pointermove", show);
  overlay.addEventListener("pointerdown", show);
  overlay.addEventListener("pointerleave", () => {
    cross.setAttribute("visibility", "hidden");
    hoverDots.forEach((dot) => dot.setAttribute("visibility", "hidden"));
    tooltip.hidden = true;
  });

  const legend = single
    ? null
    : h("div", { class: "viz-legend" }, series.map((line, si) => h("span", null, h("i", { class: `viz-key ${seriesClass(si)}` }), line.name)));
  return h(
    "figure",
    { class: "viz" },
    h("figcaption", { class: "viz__title" }, title),
    legend,
    plot,
    h("details", { class: "viz__table" }, h("summary", null, "Ver tabela"), dataTable("Mês", series)),
  );
}

/** Horizontal bars for rankings and distributions: label, bar, value at the tip. */
export function BarList(title: string, points: readonly ChartPoint[], limit = 15): HTMLElement {
  const shown = points.slice(0, limit);
  const peak = Math.max(0, ...shown.map((p) => p.value));
  const rows = shown.map((point) =>
    h(
      "div",
      { class: "viz-bar", title: `${point.label}: ${formatValue(point.value)}` },
      h("span", { class: "viz-bar__label" }, point.label),
      h("span", { class: "viz-bar__track" }, h("span", { class: "viz-bar__fill", style: `width:${peak > 0 ? Math.max((point.value / peak) * 100, 0.6) : 0}%` })),
      h("span", { class: "viz-bar__value" }, formatValue(point.value)),
    ),
  );
  return h(
    "figure",
    { class: "viz" },
    h("figcaption", { class: "viz__title" }, title),
    h("div", { class: "viz-bars", role: "list" }, rows),
    points.length > limit ? h("p", { class: "viz__note" }, `Mostrando ${limit} de ${points.length}. A tabela abaixo traz todos.`) : null,
  );
}
