/**
 * Minimal, dependency-free DOM toolkit used by every component.
 *
 * `h()` builds HTML elements, `s()` builds SVG elements. Components are plain
 * functions that return elements; nothing here knows about the application.
 */

export type ClassValue = string | false | null | undefined | readonly ClassValue[];

export type Child = Node | string | number | false | null | undefined | readonly Child[];

type EventHandlers = {
  [K in keyof HTMLElementEventMap]?: (event: HTMLElementEventMap[K]) => void;
};

export interface Props {
  class?: ClassValue;
  style?: string;
  dataset?: Record<string, string | number | boolean | undefined>;
  on?: EventHandlers;
  [attribute: string]: unknown;
}

/** Joins truthy class names (`cx("btn", active && "btn--active")`). */
export function cx(...values: ClassValue[]): string {
  const out: string[] = [];
  const walk = (value: ClassValue): void => {
    if (!value) return;
    if (typeof value === "string") out.push(value);
    else value.forEach(walk);
  };
  values.forEach(walk);
  return out.join(" ");
}

/** Attributes that must be assigned as DOM properties to stay in sync with state. */
const PROPERTY_KEYS = new Set(["value", "checked", "disabled", "hidden", "selected", "readOnly"]);

function appendChildren(parent: Node, children: readonly Child[]): void {
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    if (Array.isArray(child)) appendChildren(parent, child as readonly Child[]);
    else if (child instanceof Node) parent.appendChild(child);
    else parent.appendChild(document.createTextNode(String(child)));
  }
}

function applyProps(el: HTMLElement, props: Props): void {
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === null) continue;

    if (key === "class") {
      const className = cx(value as ClassValue);
      if (className) el.className = className;
    } else if (key === "style") {
      el.setAttribute("style", String(value));
    } else if (key === "dataset") {
      for (const [name, item] of Object.entries(value as NonNullable<Props["dataset"]>)) {
        if (item !== undefined) el.dataset[name] = String(item);
      }
    } else if (key === "on") {
      for (const [type, handler] of Object.entries(value as EventHandlers)) {
        el.addEventListener(type, handler as EventListener);
      }
    } else if (PROPERTY_KEYS.has(key)) {
      (el as unknown as Record<string, unknown>)[key] = value;
    } else if (key.startsWith("aria-") || key.startsWith("data-")) {
      el.setAttribute(key, String(value));
    } else if (value === true) {
      el.setAttribute(key, "");
    } else if (value !== false) {
      el.setAttribute(key, String(value));
    }
  }
}

/** Creates an HTML element. */
export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props?: Props | null,
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  if (props) applyProps(el, props);
  appendChildren(el, children);
  return el;
}

const SVG_NS = "http://www.w3.org/2000/svg";

export type SvgAttrs = Record<string, string | number | undefined>;
export type SvgChild = SVGElement | false | null | undefined | readonly SvgChild[];

/** Creates an SVG element. */
export function s<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs?: SvgAttrs | null,
  ...children: SvgChild[]
): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG_NS, tag);
  if (attrs) {
    for (const [name, value] of Object.entries(attrs)) {
      if (value !== undefined) el.setAttribute(name, String(value));
    }
  }
  const append = (items: readonly SvgChild[]): void => {
    for (const item of items) {
      if (!item) continue;
      if (Array.isArray(item)) append(item as readonly SvgChild[]);
      else el.appendChild(item as SVGElement);
    }
  };
  append(children);
  return el;
}

/** Replaces all children of `parent`. */
export function mount(parent: Element, ...children: Child[]): void {
  parent.replaceChildren();
  appendChildren(parent, children);
}

/** A view is an element plus an `update` hook for stateful, long-lived components. */
export interface View<P> {
  readonly el: HTMLElement;
  update(props: P): void;
}
