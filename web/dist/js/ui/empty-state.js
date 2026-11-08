import { h } from "../core/dom.js";
import { Icon } from "./icon.js";
/** Centered placeholder for screens or lists with nothing to show yet. */
export function EmptyState({ icon = "inbox", title, description }) {
    return h("div", { class: "empty-state" }, h("span", { class: "empty-state__icon" }, Icon({ name: icon, size: 26, strokeWidth: 1.5 })), h("h2", { class: "empty-state__title" }, title), h("p", { class: "empty-state__text" }, description));
}
//# sourceMappingURL=empty-state.js.map