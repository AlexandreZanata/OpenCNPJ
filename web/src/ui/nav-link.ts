import { cx, h } from "../core/dom.js";
import { Icon } from "./icon.js";
import type { IconName } from "./icons.js";

export interface NavLinkProps {
  label: string;
  href: string;
  icon: IconName;
  active?: boolean;
  /** Secondary icon aligned to the right (e.g. a chevron for expandable items). */
  trailingIcon?: IconName;
}

/** Sidebar navigation entry. */
export function NavLink({ label, href, icon, active, trailingIcon }: NavLinkProps): HTMLAnchorElement {
  return h(
    "a",
    { class: cx("nav-link", active && "is-active"), href, "aria-current": active ? "page" : undefined, title: label },
    Icon({ name: icon, size: 18, strokeWidth: 1.9, class: "nav-link__icon" }),
    h("span", { class: "nav-link__label" }, label),
    trailingIcon && Icon({ name: trailingIcon, size: 14, strokeWidth: 2, class: "nav-link__trailing" }),
  );
}
