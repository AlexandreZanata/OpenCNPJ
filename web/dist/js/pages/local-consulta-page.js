import { h } from "../core/dom.js";
import { AnswerView } from "../features/chat/answer-view.js";
import { EXAMPLE_GROUPS } from "../features/chat/examples.js";
import { ask } from "../services/local-assistant.js";
import { Icon } from "../ui/icon.js";
import { PageHero } from "../ui/page-hero.js";
const WELCOME = "Olá! Pergunte sobre empresas como em uma conversa: contagens, comparações, evolução mensal, rankings e listas com CNPJ. " +
    "Também entrego o resultado em Excel, CSV ou TXT, com as colunas que você pedir. As respostas são determinísticas e vêm da base oficial da Receita Federal.";
/** Chat with the local engine: every answer keeps the conversation context for follow-ups. */
export function LocalConsultaPage() {
    let context = null;
    let pending = null;
    const thread = h("ol", { class: "chat-thread", "aria-live": "polite", "aria-label": "Conversa" });
    const textarea = h("textarea", {
        class: "chat-composer__input",
        rows: 2,
        maxlength: 1000,
        placeholder: "Pergunte algo, por exemplo: me mande um Excel com as padarias ativas de Cuiabá/MT",
        "aria-label": "Sua pergunta",
    });
    const send = h("button", { class: "btn btn--primary chat-composer__send", type: "submit", "aria-label": "Enviar pergunta" }, Icon({ name: "send", size: 18 }), "Enviar");
    const scrollToEnd = () => {
        thread.lastElementChild?.scrollIntoView({ behavior: "smooth", block: "start" });
    };
    const assistantTurn = (...content) => h("li", { class: "chat-turn chat-turn--assistant" }, h("span", { class: "chat-avatar", "aria-hidden": "true" }, Icon({ name: "sparkles", size: 18 })), h("div", { class: "chat-bubble" }, content));
    const welcome = () => {
        thread.replaceChildren(assistantTurn(h("p", { class: "chat-lead" }, WELCOME)));
    };
    const submit = async (text) => {
        const question = text.trim();
        if (!question || pending)
            return;
        textarea.value = "";
        const request = new AbortController();
        pending = request;
        send.disabled = true;
        thread.append(h("li", { class: "chat-turn chat-turn--user" }, h("div", { class: "chat-bubble" }, question)));
        const typing = assistantTurn(h("span", { class: "chat-typing", "aria-label": "Consultando a base" }, h("i"), h("i"), h("i")));
        thread.append(typing);
        scrollToEnd();
        try {
            const reply = await ask(question, context, request.signal);
            context = reply.context;
            typing.replaceWith(assistantTurn(AnswerView(reply, question, { onFollowUp: (next) => void submit(next) })));
        }
        catch (error) {
            if (request.signal.aborted) {
                typing.remove();
            }
            else {
                typing.replaceWith(assistantTurn(h("p", { class: "chat-reject", role: "alert" }, error instanceof Error ? error.message : "Falha na consulta.")));
            }
        }
        finally {
            if (pending === request)
                pending = null;
            send.disabled = false;
            scrollToEnd();
            textarea.focus();
        }
    };
    const reset = () => {
        pending?.abort();
        pending = null;
        context = null;
        send.disabled = false;
        welcome();
        textarea.focus();
    };
    textarea.addEventListener("keydown", (event) => {
        if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            void submit(textarea.value);
        }
    });
    const composer = h("form", {
        class: "chat-composer",
        on: {
            submit: (event) => {
                event.preventDefault();
                void submit(textarea.value);
            },
        },
    }, textarea, h("div", { class: "chat-composer__actions" }, h("span", { class: "chat-muted" }, "Enter envia · Shift+Enter quebra linha"), h("button", { class: "btn chat-composer__reset", type: "button", on: { click: reset } }, Icon({ name: "x", size: 16 }), "Nova conversa"), send));
    const tabs = h("div", { class: "chat-examples__tabs", role: "tablist", "aria-label": "Exemplos por tipo de pergunta" });
    const panel = h("div", { class: "chat-examples__panel", role: "tabpanel" });
    const select = (index) => {
        const group = EXAMPLE_GROUPS[index];
        if (!group)
            return;
        tabs.querySelectorAll("button").forEach((tab, i) => tab.setAttribute("aria-selected", String(i === index)));
        panel.replaceChildren(...(group.hint ? [h("p", { class: "chat-muted" }, group.hint)] : []), h("div", { class: "chat-chips" }, group.questions.map((question) => h("button", { class: "chip", type: "button", title: "Enviar esta pergunta", on: { click: () => void submit(question) } }, question))));
    };
    EXAMPLE_GROUPS.forEach((group, i) => tabs.append(h("button", { class: "chat-examples__tab", type: "button", role: "tab", on: { click: () => select(i) } }, group.title)));
    select(0);
    welcome();
    return {
        el: h("div", { class: "page page--consulta page--local-consulta" }, PageHero({
            eyebrow: "Pesquisa com IA · motor local determinístico",
            title: ["Converse com os dados", "oficiais do CNPJ"],
            lead: "Pergunte em português, como em um chat. Cada resposta traz o número, o gráfico, as empresas com CNPJ e o arquivo em Excel, CSV ou TXT quando você pedir.",
        }), h("section", { class: "card chat", "aria-label": "Conversa com o motor de busca" }, thread, composer), h("section", { class: "card chat-examples", "aria-label": "Exemplos" }, h("h2", { class: "card__title" }, "Experimente"), tabs, panel)),
        destroy: () => {
            pending?.abort();
            pending = null;
        },
    };
}
//# sourceMappingURL=local-consulta-page.js.map