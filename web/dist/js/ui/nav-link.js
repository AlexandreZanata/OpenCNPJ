import { cx, h } from "../core/dom.js";
import { Icon } from "./icon.js";
/** Sidebar navigation entry. */
export function NavLink({ label, href, icon, active, trailingIcon }) {
    return h("a", { class: cx("nav-link", active && "is-active"), href, "aria-current": active ? "page" : undefined, title: label }, Icon({ name: icon, size: 18, strokeWidth: 1.9, class: "nav-link__icon" }), h("span", { class: "nav-link__label" }, label), trailingIcon && Icon({ name: trailingIcon, size: 14, strokeWidth: 2, class: "nav-link__trailing" }));
}
//# sourceMappingURL=nav-link.js.map