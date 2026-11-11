/** The system preference is followed until the user chooses a theme. */

import { h } from "../core/dom.js";
import { Icon } from "../ui/icon.js";

const KEY = "opencnpj.theme";
const system = window.matchMedia("(prefers-color-scheme: dark)");
type Theme = "light" | "dark";

function storedTheme(): Theme | null {
  try {
    const value = window.localStorage.getItem(KEY);
    return value === "light" || value === "dark" ? value : null;
  } catch {
    return null;
  }
}

let preference = storedTheme();

function applyTheme(): void {
  const dark = (preference ?? (system.matches ? "dark" : "light")) === "dark";
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.querySelectorAll<HTMLButtonElement>("[data-theme-toggle]").forEach((button) => render(button, dark));
}

applyTheme();
system.addEventListener("change", applyTheme);
window.addEventListener("storage", (event) => {
  if (event.key === KEY || event.key === null) {
    preference = storedTheme();
    applyTheme();
  }
});

function render(button: HTMLButtonElement, dark: boolean): void {
  button.setAttribute("aria-pressed", String(dark));
  button.title = dark ? "Mudar para tema claro" : "Mudar para tema escuro";
  button.replaceChildren(Icon({ name: dark ? "sun" : "moon", size: 18 }));
}

export function ThemeToggle(): HTMLButtonElement {
  const button = h("button", {
    class: "nav-link nav-link--icon",
    type: "button",
    "aria-label": "Tema escuro",
    "data-theme-toggle": "",
    on: {
      click: () => {
        preference = document.documentElement.dataset.theme === "dark" ? "light" : "dark";
        try {
          window.localStorage.setItem(KEY, preference);
        } catch {
          /* Keep the selection for this tab. */
        }
        applyTheme();
      },
    },
  });

  render(button, document.documentElement.dataset.theme === "dark");
  return button;
}
