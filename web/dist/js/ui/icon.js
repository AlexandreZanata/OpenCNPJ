import { cx, s } from "../core/dom.js";
import { ICONS } from "./icons.js";
/** Renders a registry icon as an inline SVG that inherits `currentColor`. */
export function Icon({ name, size = 18, strokeWidth = 1.75, class: className, label }) {
    const svg = s("svg", {
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
    }, ICONS[name].map(([tag, attrs]) => s(tag, attrs)));
    if (label) {
        svg.setAttribute("role", "img");
        svg.setAttribute("aria-label", label);
    }
    else {
        svg.setAttribute("aria-hidden", "true");
    }
    return svg;
}
//# sourceMappingURL=icon.js.map