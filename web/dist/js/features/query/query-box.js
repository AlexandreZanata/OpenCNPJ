import { h } from "../../core/dom.js";
import { Button } from "../../ui/button.js";
import { Chip } from "../../ui/chip.js";
import { Icon } from "../../ui/icon.js";
/** Natural-language search box with example chips. */
export function QueryBox(options) {
    const textarea = h("textarea", {
        class: "query-box__input",
        id: "query-input",
        rows: 2,
        spellcheck: false,
        placeholder: "Descreva as empresas que você procura…",
        "aria-label": "Pergunta em linguagem natural",
        on: {
            keydown: (event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                    event.preventDefault();
                    options.onSubmit(textarea.value);
                }
            },
        },
    });
    const clear = h("button", { class: "query-box__clear", type: "button", "aria-label": "Limpar consulta", on: { click: options.onClear } }, Icon({ name: "x", size: 18, strokeWidth: 1.5 }));
    const submit = Button({
        label: "Consultar com IA",
        icon: "send",
        variant: "primary",
        type: "submit",
        class: "query-box__submit",
    });
    const form = h("form", {
        class: "query-box__field",
        on: {
            submit: (event) => {
                event.preventDefault();
                options.onSubmit(textarea.value);
            },
        },
    }, Icon({ name: "sparkles", size: 28, strokeWidth: 1.8, class: "query-box__icon" }), textarea, clear, submit);
    const examples = h("div", { class: "query-box__examples" }, h("span", { class: "query-box__examples-label" }, "Exemplos:"), h("div", { class: "query-box__chips" }, options.examples.map((text) => Chip({ label: text, onClick: () => options.onExample(text) }))));
    const el = h("section", { class: "card query-box", "aria-label": "Consulta com IA" }, form, examples);
    // The textarea owns what the user is typing; the controlled `value` is only
    // pushed in when it changes (example chip, clear, new query).
    let applied = null;
    textarea.addEventListener("input", () => {
        applied = textarea.value;
        clear.hidden = textarea.value.length === 0;
    });
    return {
        el,
        update: ({ value, loading }) => {
            if (value !== applied) {
                applied = value;
                textarea.value = value;
                clear.hidden = value.length === 0;
            }
            submit.disabled = loading;
            submit.classList.toggle("is-loading", loading);
            el.setAttribute("aria-busy", String(loading));
        },
    };
}
//# sourceMappingURL=query-box.js.map