import { cx, s, type ClassValue } from "../core/dom.js";
import { ICONS, type IconName } from "./icons.js";

export interface IconProps {
  name: IconName;
  /** Pixel size (width and height). */
  size?: number;
  strokeWidth?: number;
  class?: ClassValue;
  /** Accessible name. Icons are decorative (`aria-hidden`) unless a label is given. */
  label?: string;
}

/** Renders a registry icon as an inline SVG that inherits `currentColor`. */
export function Icon({ name, size = 18, strokeWidth = 1.75, class: className, label }: IconProps): SVGSVGElement {
  const svg = s(
    "svg",
    {
      class: cx("icon", className),
      width: size,
      height: size,
      viewBox: "0 0 24 24",
      fill: "none",
      stroke: "currentColor",
      "stroke-width": strokeWidth,
      "stroke-linecap": "round",
      "stroke-linejoin": "round",
      focusable: "false",
    },
    ICONS[name].map(([tag, attrs]) => s(tag, attrs)),
  );
  if (label) {
    svg.setAttribute("role", "img");
    svg.setAttribute("aria-label", label);
  } else {
    svg.setAttribute("aria-hidden", "true");
  }
  return svg;
}
