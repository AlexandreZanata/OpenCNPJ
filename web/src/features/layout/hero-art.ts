import { h, s, type SvgChild } from "../../core/dom.js";

/** Decorative glass towers behind the page header (pure SVG, no image assets). */

const W = 338;
const H = 150;

function face(points: string, fill: string, grid: "light" | "dark" | "none" = "light"): SvgChild {
  return [
    s("polygon", { points, fill }),
    grid !== "none" && s("polygon", { points, fill: `url(#glass-${grid})` }),
  ];
}

function edge(points: string): SvgChild {
  return s("polyline", { points, fill: "none", stroke: "#ffffff", "stroke-width": 1.4, "stroke-linejoin": "round", opacity: 0.9 });
}

export function HeroArt(): HTMLElement {
  const gradient = (id: string, from: string, to: string): SvgChild =>
    s("linearGradient", { id, x1: "0", y1: "0", x2: "0", y2: "1" }, s("stop", { offset: "0", "stop-color": from }), s("stop", { offset: "1", "stop-color": to }));

  const pattern = (id: string, line: string, hLine: string): SvgChild =>
    s(
      "pattern",
      { id, width: 6, height: 11, patternUnits: "userSpaceOnUse" },
      s("line", { x1: 0.5, y1: 0, x2: 0.5, y2: 11, stroke: line, "stroke-width": 0.8 }),
      s("line", { x1: 0, y1: 0.5, x2: 6, y2: 0.5, stroke: hLine, "stroke-width": 0.6 }),
    );

  const svg = s(
    "svg",
    { viewBox: `0 0 ${W} ${H}`, preserveAspectRatio: "xMaxYMin meet", "aria-hidden": "true", focusable: "false" },
    s(
      "defs",
      null,
      gradient("glass-a-light", "#f4f7f9", "#dfe8ee"),
      gradient("glass-a-dark", "#a4b6c3", "#c9d6df"),
      gradient("glass-b-light", "#e6edf1", "#cbd8e0"),
      gradient("glass-b-dark", "#9fb2bf", "#b9c9d3"),
      gradient("glass-c-light", "#dde7ed", "#bccbd5"),
      gradient("glass-c-dark", "#92a7b6", "#a9bbc7"),
      pattern("glass-light", "rgba(255,255,255,0.75)", "rgba(110,140,160,0.18)"),
      pattern("glass-dark", "rgba(255,255,255,0.28)", "rgba(40,70,95,0.16)"),
    ),
    // Tower A
    face("137,17 1,150 137,150", "url(#glass-a-light)"),
    face("137,17 201,85 201,150 137,150 137,127", "url(#glass-a-dark)", "dark"),
    // Tower B
    face("277,39 204,88 204,150 277,150", "url(#glass-b-light)"),
    face("277,39 338,100 338,150 277,150", "url(#glass-b-dark)", "dark"),
    // Foreground wedge
    face("202,86 121,150 202,150", "url(#glass-c-light)"),
    face("202,86 264,150 202,150", "url(#glass-c-dark)", "dark"),
    edge("1,150 137,17 201,85"),
    edge("204,88 277,39 338,100"),
    edge("121,150 202,86 264,150"),
  );

  return h("div", { class: "hero-art", "aria-hidden": "true" }, svg);
}
