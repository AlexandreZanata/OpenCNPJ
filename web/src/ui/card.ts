import { cx, h, type Child, type ClassValue } from "../core/dom.js";

export interface CardProps {
  class?: ClassValue;
  title?: string;
  /** Optional element id of the card heading, for `aria-labelledby`. */
  titleId?: string;
}

/** Base surface used by every panel: metrics, charts, banners. */
export function Card(props: CardProps, ...children: Child[]): HTMLElement {
  return h(
    "section",
    { class: cx("card", props.class), "aria-labelledby": props.title ? props.titleId : undefined },
    props.title && h("h2", { class: "card__title", id: props.titleId }, props.title),
    ...children,
  );
}
