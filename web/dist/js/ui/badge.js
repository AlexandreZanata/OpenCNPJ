import { h } from "../core/dom.js";
/** Small rounded status label. */
export function Badge({ label, tone = "neutral" }) {
    return h("span", { class: `badge badge--${tone}` }, label);
}
//# sourceMappingURL=badge.js.map