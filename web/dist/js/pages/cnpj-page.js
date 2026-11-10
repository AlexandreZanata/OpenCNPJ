/** CNPJ lookup: a 14-character CNPJ shows one establishment, an 8-character root the company. */
import { errorMessage, getJson } from "../core/api.js";
import { h } from "../core/dom.js";
import { navigate } from "../core/router.js";
import { CompanyView } from "../features/company/company-view.js";
import { Button, EmptyState, Input, Spinner } from "../ui/kit.js";
export function CnpjPage(param) {
    const input = Input({ value: param, placeholder: "00.000.000/0000-00 ou os 8 primeiros dígitos", inputmode: "text", class: "input--lg" });
    const result = h("div", { class: "page__result" });
    const controller = new AbortController();
    const show = (raw) => {
        const id = raw.replace(/[^0-9A-Za-z]/g, "");
        if (id.length !== 14 && id.length !== 8) {
            result.replaceChildren(EmptyState("search", "Digite os 14 caracteres do CNPJ, ou os 8 da raiz para ver a empresa."));
            return;
        }
        result.replaceChildren(Spinner("Buscando o CNPJ"));
        getJson(`/api/v1/cnpj/${encodeURIComponent(id)}`, controller.signal)
            .then((record) => result.replaceChildren(h("div", { class: "card" }, CompanyView(record))))
            .catch((error) => {
            const message = errorMessage(error);
            if (message)
                result.replaceChildren(EmptyState("info", message));
        });
    };
    const form = h("form", {
        class: "lookup",
        on: {
            submit: (event) => {
                event.preventDefault();
                const id = input.value.replace(/[^0-9A-Za-z]/g, "");
                if (id === param)
                    show(id);
                else
                    navigate("cnpj", id);
            },
        },
    }, input, Button({ label: "Consultar", icon: "search", variant: "primary", type: "submit" }));
    if (param)
        show(param);
    else
        result.replaceChildren(EmptyState("building", "Consulte qualquer CNPJ do Brasil.", "Cadastro, atividade, endereço, contatos e quadro societário, direto da base oficial."));
    return {
        el: h("div", { class: "page page--narrow" }, h("h1", { class: "page__title" }, "Consulta de CNPJ"), form, result),
        destroy: () => controller.abort(),
    };
}
//# sourceMappingURL=cnpj-page.js.map