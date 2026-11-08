import { h } from "../core/dom.js";
import { Icon } from "./icon.js";
/** Verified-filter tag: ✓ **Label:** value. */
export function FilterTag({ label, value }) {
    return h("span", { class: "filter-tag" }, Icon({ name: "check", size: 14, strokeWidth: 2.25, class: "filter-tag__icon" }), h("strong", { class: "filter-tag__label" }, `${label}:`), h("span", { class: "filter-tag__value" }, ` ${value}`));
}
//# sourceMappingURL=filter-tag.js.map