/** Small set of interface pieces shared by every page. */
import { cx, h } from "../core/dom.js";
import { formatInteger } from "../core/format.js";
import { Icon } from "./icon.js";
export function Button(props) {
    const { label, icon, variant = "secondary", size = "md", type = "button", title, disabled, onClick } = props;
    return h("button", {
        class: cx("btn", `btn--${variant}`, size === "sm" && "btn--sm", !label && "btn--icon", props.class),
        type,
        title,
        disabled,
        "aria-label": label ? undefined : title,
        on: onClick ? { click: onClick } : undefined,
    }, icon ? Icon({ name: icon, size: size === "sm" ? 15 : 17 }) : null, label ?? null);
}
/** Runs `task` with the button disabled, so a slow request cannot be sent twice. */
export async function busy(button, task) {
    if (button.disabled)
        return;
    button.disabled = true;
    button.classList.add("is-busy");
    try {
        await task();
    }
    finally {
        button.disabled = false;
        button.classList.remove("is-busy");
    }
}
export function Chip(label, onClick, title) {
    return h("button", { class: "chip", type: "button", title, on: { click: onClick } }, label);
}
export function Badge(label, tone = "neutral") {
    return h("span", { class: `badge badge--${tone}` }, label);
}
export function Spinner(label = "Carregando") {
    return h("span", { class: "spinner", role: "status", "aria-label": label }, h("i"), h("i"), h("i"));
}
export function EmptyState(icon, title, hint) {
    return h("div", { class: "empty" }, Icon({ name: icon, size: 22 }), h("strong", null, title), hint ? h("span", null, hint) : null);
}
let fieldId = 0;
/** A labelled form control. */
export function Field({ label, hint, control, class: className }) {
    if (!control.id)
        control.id = `field-${++fieldId}`;
    return h("div", { class: cx("field", className) }, h("label", { class: "field__label", for: control.id }, label), control, hint ? h("span", { class: "field__hint" }, hint) : null);
}
export function Input(props = {}) {
    const input = h("input", {
        class: cx("input", props.class),
        type: props.type ?? "text",
        name: props.name,
        value: props.value ?? "",
        placeholder: props.placeholder,
        autocomplete: props.autocomplete ?? "off",
        inputmode: props.inputmode,
        maxlength: props.maxlength,
        required: props.required,
        spellcheck: "false",
    });
    if (props.onInput)
        input.addEventListener("input", () => props.onInput?.(input.value));
    return input;
}
export function Select(options, value = "", onChange) {
    const select = h("select", { class: "input select" }, options.map(([optionValue, label]) => h("option", { value: optionValue, selected: optionValue === value }, label)));
    if (onChange)
        select.addEventListener("change", () => onChange(select.value));
    return select;
}
export function Checkbox(label, checked = false, onChange) {
    const input = h("input", { type: "checkbox", checked });
    if (onChange)
        input.addEventListener("change", () => onChange(input.checked));
    return h("label", { class: "check" }, input, h("span", null, label));
}
/** Used / limit bar; `limit === null` means unlimited. */
export function Meter(label, used, limit) {
    const ratio = limit ? Math.min(1, used / limit) : 0;
    return h("div", { class: "meter" }, h("div", { class: "meter__head" }, h("span", null, label), h("strong", null, limit === null ? `${formatInteger(used)} · sem limite` : `${formatInteger(used)} de ${formatInteger(limit)}`)), h("div", { class: "meter__track", role: "progressbar", "aria-valuemin": 0, "aria-valuemax": limit ?? 0, "aria-valuenow": used }, h("div", { class: cx("meter__fill", ratio >= 0.9 && "meter__fill--high"), style: `width:${(ratio * 100).toFixed(1)}%` })));
}
// ----- transient surfaces --------------------------------------------------------------------
let toastHost = null;
/** A short message at the bottom of the screen. */
export function toast(message, tone = "info") {
    if (!message)
        return;
    if (!toastHost) {
        toastHost = h("div", { class: "toasts", "aria-live": "polite" });
        document.body.appendChild(toastHost);
    }
    const item = h("div", { class: `toast toast--${tone}`, role: tone === "error" ? "alert" : "status" }, message);
    toastHost.appendChild(item);
    setTimeout(() => item.classList.add("is-leaving"), 4200);
    setTimeout(() => item.remove(), 4600);
}
function overlay(kind, title, body, onClose) {
    const previous = document.activeElement;
    const close = () => {
        document.removeEventListener("keydown", onKey);
        root.remove();
        previous?.focus?.();
        onClose?.();
    };
    const onKey = (event) => {
        if (event.key === "Escape")
            close();
    };
    const panel = h("section", { class: `overlay__panel overlay__panel--${kind}`, role: "dialog", "aria-modal": "true", "aria-label": title, tabindex: -1 }, h("header", { class: "overlay__head" }, h("h2", null, title), Button({ icon: "x", variant: "ghost", size: "sm", title: "Fechar", onClick: close })), h("div", { class: "overlay__body" }, body));
    const root = h("div", {
        class: `overlay overlay--${kind}`,
        on: {
            mousedown: (event) => {
                if (event.target === root)
                    close();
            },
        },
    }, panel);
    document.body.appendChild(root);
    document.addEventListener("keydown", onKey);
    panel.focus();
    return { close };
}
/** Panel sliding from the right, for details that keep the page behind in view. */
export function openDrawer(title, body, onClose) {
    return overlay("drawer", title, body, onClose);
}
/** Centered dialog for a short form or a confirmation. */
export function openModal(title, body, onClose) {
    return overlay("modal", title, body, onClose);
}
/** Asks before something that cannot be undone. Resolves `true` when confirmed. */
export function confirmAction(title, message, confirmLabel, danger = false) {
    return new Promise((resolve) => {
        let answer = false;
        const modal = openModal(title, [
            h("p", { class: "muted" }, message),
            h("div", { class: "actions actions--end" }, Button({ label: "Cancelar", onClick: () => modal.close() }), Button({
                label: confirmLabel,
                variant: danger ? "danger" : "primary",
                onClick: () => {
                    answer = true;
                    modal.close();
                },
            })),
        ], () => resolve(answer));
    });
}
/** Copies text and says so. */
export async function copyText(text, done = "Copiado.") {
    try {
        await navigator.clipboard.writeText(text);
        toast(done);
    }
    catch {
        toast("Não consegui copiar. Selecione o texto e copie manualmente.", "error");
    }
}
/** A plain data table inside a scrollable frame. */
export function DataTable(columns, rows, onRow) {
    return h("div", { class: "table-wrap" }, h("table", { class: "table" }, h("thead", null, h("tr", null, columns.map((column) => h("th", { scope: "col", class: column.class }, column.header)))), h("tbody", null, rows.map((row) => h("tr", onRow ? { class: "is-clickable", tabindex: 0, on: { click: () => onRow(row), keydown: (event) => event.key === "Enter" && onRow(row) } } : null, columns.map((column) => h("td", { class: column.class }, column.cell(row))))))));
}
//# sourceMappingURL=kit.js.map