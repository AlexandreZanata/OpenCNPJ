import { cx, h } from "../core/dom.js";
import { Icon } from "./icon.js";
/** The single button component: every clickable control in the app is built from it. */
export function Button(props) {
    const { label, icon, trailingIcon, variant = "secondary", badge, type = "button", disabled } = props;
    return h("button", {
        class: cx("btn", `btn--${variant}`, props.class),
        type,
        disabled,
        "aria-label": props.ariaLabel,
        "aria-expanded": props.ariaExpanded,
        "aria-haspopup": props.ariaHasPopup,
        on: props.onClick ? { click: props.onClick } : undefined,
    }, icon && Icon({ name: icon, size: 16, class: "btn__icon" }), label && h("span", { class: "btn__label" }, label), badge !== undefined && h("span", { class: "btn__badge" }, badge), trailingIcon && Icon({ name: trailingIcon, size: 14, class: "btn__icon btn__icon--trailing" }));
}
//# sourceMappingURL=button.js.map