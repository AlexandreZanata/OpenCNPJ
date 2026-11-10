/** Conversation with the assistant. Every answer keeps the context for follow-ups. */
import { ApiError, errorMessage, request } from "../core/api.js";
import { h } from "../core/dom.js";
import { href } from "../core/router.js";
import { session } from "../core/session.js";
import { AnswerView } from "../features/chat/answer-view.js";
import { EXAMPLE_GROUPS } from "../features/chat/examples.js";
import { Icon } from "../ui/icon.js";
import { Button, Chip, EmptyState } from "../ui/kit.js";
export function AssistantPage() {
    if (session().platform?.assistant === false) {
        return {
            el: h("div", { class: "page page--narrow" }, EmptyState("message", "O assistente não está habilitado neste servidor.", h("a", { href: href("busca") }, "Use a busca por filtros"))),
        };
    }
    let context = null;
    let pending = null;
    const thread = h("ol", { class: "thread", "aria-live": "polite", "aria-label": "Conversa" });
    const textarea = h("textarea", {
        class: "composer__input",
        rows: 1,
        maxlength: 500,
        placeholder: "Pergunte: dentistas em Cuiabá com telefone, novas empresas em Sinop, um CNPJ…",
        "aria-label": "Sua pergunta",
    });
    const send = h("button", { class: "composer__send", type: "submit", title: "Enviar (Enter)", "aria-label": "Enviar pergunta" }, Icon({ name: "arrow-up", size: 18 }));
    const fresh = Button({ label: "Nova conversa", icon: "plus", variant: "ghost", size: "sm", onClick: () => reset() });
    const grow = () => {
        textarea.style.height = "auto";
        textarea.style.height = `${Math.min(textarea.scrollHeight, 180)}px`;
    };
    const toEnd = () => thread.lastElementChild?.scrollIntoView({ behavior: "smooth", block: "start" });
    const assistantTurn = (...content) => h("li", { class: "turn turn--assistant" }, content);
    const submit = async (text) => {
        const question = text.trim();
        if (!question || pending)
            return;
        textarea.value = "";
        grow();
        const controller = new AbortController();
        pending = controller;
        send.disabled = true;
        root.classList.add("has-thread");
        thread.append(h("li", { class: "turn turn--user" }, h("p", null, question)));
        const typing = assistantTurn(h("span", { class: "spinner", role: "status", "aria-label": "Consultando a base" }, h("i"), h("i"), h("i")));
        thread.append(typing);
        toEnd();
        try {
            const body = context ? { question, context } : { question };
            const reply = (await (await request("/api/v1/assistant/ask", { method: "POST", body, signal: controller.signal })).json());
            context = reply.context;
            typing.replaceWith(assistantTurn(AnswerView(reply, question, { onFollowUp: (next) => void submit(next) })));
        }
        catch (error) {
            if (controller.signal.aborted) {
                typing.remove();
            }
            else {
                const quota = error instanceof ApiError && error.code === "quota_exceeded";
                typing.replaceWith(assistantTurn(h("div", { class: "answer" }, h("p", { class: "note note--error", role: "alert" }, Icon({ name: "info", size: 15 }), h("span", null, errorMessage(error) || "Falha na consulta.")), quota ? h("a", { href: href("conta") }, "Ver meu plano e uso") : null)));
            }
        }
        finally {
            if (pending === controller)
                pending = null;
            send.disabled = false;
            toEnd();
            textarea.focus();
        }
    };
    const reset = () => {
        pending?.abort();
        pending = null;
        context = null;
        send.disabled = false;
        thread.replaceChildren();
        root.classList.remove("has-thread");
        textarea.focus();
    };
    textarea.addEventListener("input", grow);
    textarea.addEventListener("keydown", (event) => {
        if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
            event.preventDefault();
            void submit(textarea.value);
        }
    });
    const composer = h("form", {
        class: "composer",
        on: {
            submit: (event) => {
                event.preventDefault();
                void submit(textarea.value);
            },
        },
    }, textarea, send);
    const competence = session().platform?.competence;
    const welcome = h("div", { class: "welcome" }, h("h1", { class: "welcome__title" }, "O que você quer encontrar?"), h("p", { class: "welcome__lead" }, "Pergunte em português, como em uma conversa. Cada resposta sai dos dados oficiais do CNPJ", competence ? ` (Receita Federal, ${competence})` : "", ", sem estimativas."));
    const examples = h("div", { class: "examples" }, EXAMPLE_GROUPS.map((group) => h("section", { class: "examples__group" }, h("h2", null, group.title), h("div", { class: "chips" }, group.questions.map((question) => Chip(question, () => void submit(question)))))));
    const root = h("div", { class: "page page--chat" }, welcome, thread, h("div", { class: "dock" }, h("div", { class: "dock__bar" }, fresh), composer, h("p", { class: "dock__hint" }, "Enter envia · Shift+Enter quebra a linha")), examples);
    return {
        el: root,
        resume: () => textarea.focus(),
        destroy: () => {
            pending?.abort();
            pending = null;
        },
    };
}
//# sourceMappingURL=assistant-page.js.map