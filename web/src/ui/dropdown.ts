import { h, type View } from "../core/dom.js";
import { Button, type ButtonProps } from "./button.js";
import { Icon } from "./icon.js";

export interface MenuItem {
  id: string;
  label: string;
  /** When defined, the item behaves as a checkbox. */
  checked?: boolean;
  /** Keep the menu open after selecting (checkbox lists). */
  keepOpen?: boolean;
  onSelect: () => void;
}

export interface DropdownProps {
  items: readonly MenuItem[];
}

export interface DropdownOptions {
  trigger: Omit<ButtonProps, "onClick" | "ariaExpanded" | "ariaHasPopup">;
  items: readonly MenuItem[];
  /** Menu edge aligned with the trigger. */
  align?: "start" | "end";
}

/** Button that opens a popover menu. Closes on outside click, Escape and selection. */
export function Dropdown(options: DropdownOptions): View<DropdownProps> {
  const menu = h("div", { class: "dropdown__menu", role: "menu", hidden: true });
  const root = h("div", { class: `dropdown dropdown--${options.align ?? "end"}` });
  let open = false;
  let trigger: HTMLButtonElement;

  const setOpen = (next: boolean): void => {
    open = next;
    menu.hidden = !next;
    trigger.setAttribute("aria-expanded", String(next));
  };

  const onDocumentClick = (event: MouseEvent): void => {
    if (open && !root.contains(event.target as Node)) setOpen(false);
  };
  const onKeydown = (event: KeyboardEvent): void => {
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

  const render = (items: readonly MenuItem[]): void => {
    menu.replaceChildren(
      ...items.map((item) => {
        const checkable = item.checked !== undefined;
        return h(
          "button",
          {
            class: "dropdown__item",
            type: "button",
            role: checkable ? "menuitemcheckbox" : "menuitem",
            "aria-checked": checkable ? item.checked : undefined,
            on: {
              click: () => {
                item.onSelect();
                if (!item.keepOpen) setOpen(false);
              },
            },
          },
          h("span", { class: "dropdown__check" }, item.checked ? Icon({ name: "check", size: 14, strokeWidth: 2.25 }) : null),
          h("span", null, item.label),
        );
      }),
    );
  };

  root.append(trigger, menu);
  render(options.items);

  return { el: root, update: ({ items }) => render(items) };
}
