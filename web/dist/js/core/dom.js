/**
 * Minimal, dependency-free DOM toolkit used by every component.
 *
 * `h()` builds HTML elements, `s()` builds SVG elements. Components are plain
 * functions that return elements; nothing here knows about the application.
 */
/** Joins truthy class names (`cx("btn", active && "btn--active")`). */
export function cx(...values) {
    const out = [];
    const walk = (value) => {
        if (!value)
            return;
        if (typeof value === "string")
            out.push(value);
        else
            value.forEach(walk);
    };
    values.forEach(walk);
    return out.join(" ");
}
/** Attributes that must be assigned as DOM properties to stay in sync with state. */
const PROPERTY_KEYS = new Set(["value", "checked", "disabled", "hidden", "selected", "readOnly"]);
function appendChildren(parent, children) {
    for (const child of children) {
        if (child === null || child === undefined || child === false)
            continue;
        if (Array.isArray(child))
            appendChildren(parent, child);
        else if (child instanceof Node)
            parent.appendChild(child);
        else
            parent.appendChild(document.createTextNode(String(child)));
    }
}
function applyProps(el, props) {
    for (const [key, value] of Object.entries(props)) {
        if (value === undefined || value === null)
            continue;
        if (key === "class") {
            const className = cx(value);
            if (className)
                el.className = className;
        }
        else if (key === "style") {
            el.setAttribute("style", String(value));
        }
        else if (key === "dataset") {
            for (const [name, item] of Object.entries(value)) {
                if (item !== undefined)
                    el.dataset[name] = String(item);
            }
        }
        else if (key === "on") {
            for (const [type, handler] of Object.entries(value)) {
                el.addEventListener(type, handler);
            }
        }
        else if (PROPERTY_KEYS.has(key)) {
            el[key] = value;
        }
        else if (key.startsWith("aria-") || key.startsWith("data-")) {
            el.setAttribute(key, String(value));
        }
        else if (value === true) {
            el.setAttribute(key, "");
        }
        else if (value !== false) {
            el.setAttribute(key, String(value));
        }
    }
}
/** Creates an HTML element. */
export function h(tag, props, ...children) {
    const el = document.createElement(tag);
    if (props)
        applyProps(el, props);
    appendChildren(el, children);
    return el;
}
const SVG_NS = "http://www.w3.org/2000/svg";
/** Creates an SVG element. */
export function s(tag, attrs, ...children) {
    const el = document.createElementNS(SVG_NS, tag);
    if (attrs) {
        for (const [name, value] of Object.entries(attrs)) {
            if (value !== undefined)
                el.setAttribute(name, String(value));
        }
    }
    const append = (items) => {
        for (const item of items) {
            if (!item)
                continue;
            if (Array.isArray(item))
                append(item);
            else
                el.appendChild(item);
        }
    };
    append(children);
    return el;
}
/** Replaces all children of `parent`. */
export function mount(parent, ...children) {
    parent.replaceChildren();
    appendChildren(parent, children);
}
//# sourceMappingURL=dom.js.map