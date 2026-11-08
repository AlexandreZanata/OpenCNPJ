import { h } from "../core/dom.js";
import { Button } from "./button.js";
import { Icon } from "./icon.js";
/** Button that opens a popover menu. Closes on outside click, Escape and selection. */
export function Dropdown(options) {
    const menu = h("div", { class: "dropdown__menu", role: "menu", hidden: true });
    const root = h("div", { class: `dropdown dropdown--${options.align ?? "end"}` });
    let open = false;
    let trigger;
    const setOpen = (next) => {
        open = next;
        menu.hidden = !next;
        trigger.setAttribute("aria-expanded", String(next));
    };
    const onDocumentClick = (event) => {
        if (open && !root.contains(event.target))
            setOpen(false);
    };
    const onKeydown = (event) => {
        if (open && event.key === "Escape") {
            setOpen(false);
            trigger.focus();
        }
    };
    document.addEventListener("click", onDocumentClick);
    document.addEventListener("keydown", onKeydown);
    trigger = Button({
        ...options.trigger,
        ariaHasPopup: "menu",
        ariaExpanded: false,
        onClick: () => setOpen(!open),
    });
    const render = (items) => {
        menu.replaceChildren(...items.map((item) => {
            const checkable = item.checked !== undefined;
            return h("button", {
                class: "dropdown__item",
                type: "button",
                role: checkable ? "menuitemcheckbox" : "menuitem",
                "aria-checked": checkable ? item.checked : undefined,
                on: {
                    click: () => {
                        item.onSelect();
                        if (!item.keepOpen)
                            setOpen(false);
                    },
                },
            }, h("span", { class: "dropdown__check" }, item.checked ? Icon({ name: "check", size: 14, strokeWidth: 2.25 }) : null), h("span", null, item.label));
        }));
    };
    root.append(trigger, menu);
    render(options.items);
    return { el: root, update: ({ items }) => render(items) };
}
//# sourceMappingURL=dropdown.js.map