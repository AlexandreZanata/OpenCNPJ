import { h, s } from "../core/dom.js";

/** Wordmark: a green tile with a magnifier-like ring and the product name. */
export function Logo(): HTMLElement {
  return h(
    "span",
    { class: "logo" },
    s(
      "svg",
      { class: "logo__mark", width: 26, height: 26, viewBox: "0 0 26 26", "aria-hidden": "true" },
      s("rect", { x: 0, y: 0, width: 26, height: 26, rx: 7, fill: "var(--accent)" }),
      s("circle", { cx: 12, cy: 12, r: 5.2, fill: "none", stroke: "var(--accent-ink)", "stroke-width": 2.2 }),
      s("path", { d: "m16 16 4 4", stroke: "var(--accent-ink)", "stroke-width": 2.2, "stroke-linecap": "round" }),
    ),
    h("span", { class: "logo__name" }, "OpenCNPJ"),
  );
}
