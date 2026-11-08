import { h } from "../core/dom.js";
/** Pill-shaped suggestion (query examples). */
export function Chip({ label, onClick }) {
    return h("button", { class: "chip", type: "button", on: onClick ? { click: onClick } : undefined }, label);
}
//# sourceMappingURL=chip.js.map