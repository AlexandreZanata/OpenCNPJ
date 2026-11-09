import { cx, h } from "../core/dom.js";
/** Base surface used by every panel: metrics, charts, banners. */
export function Card(props, ...children) {
    return h("section", { class: cx("card", props.class), "aria-labelledby": props.title ? props.titleId : undefined }, props.title && h("h2", { class: "card__title", id: props.titleId }, props.title), ...children);
}
//# sourceMappingURL=card.js.map