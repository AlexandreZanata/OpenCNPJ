/** Conversation with the assistant. Every answer keeps the context for follow-ups. */
import { ApiError, errorMessage, getJson, request, sendJson } from "../core/api.js";
import { h } from "../core/dom.js";
import { href } from "../core/router.js";
import { session } from "../core/session.js";
import { AnswerView } from "../features/chat/answer-view.js";
import { EXAMPLE_GROUPS } from "../features/chat/examples.js";
import { Icon } from "../ui/icon.js";
import { Button, Chip, EmptyState, Field, Input, busy, confirmAction, openModal, toast } from "../ui/kit.js";
const CONVERSATIONS = "/api/v1/conversations";
export function AssistantPage() {
    if (session().platform?.assistant === false) {
        return {
            el: h("div", { class: "page page--narrow" }, EmptyState("message", "O assistente não está habilitado neste servidor.", h("a", { href: href("busca") }, "Use a busca por filtros"))),
        };
    }
    const signedIn = Boolean(session().token);
    const preview = session().platform?.auth === "trial";
    let context = null;
    let pending = null;
    let loading = null;
    let disposed = false;
    let active = null;
    let revision = 0;
    let conversations = [];
    const history = h("ul", { class: "conversation-list", "aria-label": "Histórico de conversas" });
    const historyStatus = h("p", { class: "conversation-status muted", role: "status" });
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
    const userTurn = (question) => h("li", { class: "turn turn--user" }, h("p", null, question));
    const answerTurn = (reply, question) => assistantTurn(AnswerView(reply, question, { onFollowUp: (next) => void submit(next) }));
    const renderHistory = () => {
        history.replaceChildren(...conversations.map((item) => h("li", { class: `conversation-item${item.id === active ? " conversation-item--active" : ""}` }, h("button", {
            class: "conversation-item__name",
            type: "button",
            title: item.title,
            "aria-current": item.id === active ? "true" : undefined,
            on: { click: () => void openSaved(item.id) },
        }, item.title), Button({ icon: "pencil", title: `Renomear: ${item.title}`, size: "sm", variant: "ghost", onClick: () => rename(item) }), Button({ icon: "trash", title: `Excluir: ${item.title}`, size: "sm", variant: "ghost", onClick: () => void remove(item) }))));
    };
    const updateHistory = (item) => {
        conversations = [item, ...conversations.filter((entry) => entry.id !== item.id)]
            .sort((left, right) => right.updated_at - left.updated_at || right.id - left.id);
        historyStatus.textContent = "";
        renderHistory();
    };
    const refreshHistory = async () => {
        try {
            const saved = await getJson(CONVERSATIONS);
            if (disposed)
                return;
            conversations = saved;
            historyStatus.textContent = saved.length ? "" : "Suas conversas salvas aparecem aqui.";
            renderHistory();
        }
        catch (error) {
            if (!disposed)
                historyStatus.textContent = errorMessage(error);
        }
    };
    const initialHistory = signedIn ? refreshHistory() : Promise.resolve();
    const openSaved = async (id) => {
        reset(false);
        const controller = new AbortController();
        loading = controller;
        send.disabled = true;
        historyStatus.textContent = "Carregando conversa…";
        try {
            const turns = await getJson(`${CONVERSATIONS}/${id}`, controller.signal);
            if (loading !== controller)
                return;
            active = id;
            revision = turns.length;
            historyStatus.textContent = "";
            root.classList.toggle("has-thread", turns.length > 0);
            thread.replaceChildren(...turns.flatMap(({ question, reply }) => [userTurn(question), answerTurn(reply, question)]));
            renderHistory();
            toEnd();
        }
        catch (error) {
            if (loading === controller)
                historyStatus.textContent = errorMessage(error);
        }
        finally {
            if (loading === controller) {
                loading = null;
                send.disabled = false;
                textarea.focus();
            }
        }
    };
    const rename = (item) => {
        const title = Input({ required: true, maxlength: 120, value: item.title });
        const save = Button({ label: "Salvar", variant: "primary", type: "submit" });
        const form = h("form", {
            class: "auth__form",
            on: {
                submit: (event) => {
                    event.preventDefault();
                    void busy(save, async () => {
                        try {
                            const updated = await sendJson("PATCH", `${CONVERSATIONS}/${item.id}`, { title: title.value.trim() });
                            if (!disposed)
                                updateHistory(updated);
                            dialog.close();
                        }
                        catch (error) {
                            toast(errorMessage(error), "error");
                        }
                    });
                },
            },
        }, Field({ label: "Título", control: title }), save);
        const dialog = openModal("Renomear conversa", form);
        title.select();
    };
    const remove = async (item) => {
        if (!(await confirmAction("Excluir conversa?", `“${item.title}” e suas consultas serão excluídas.`, "Excluir", true)))
            return;
        try {
            await sendJson("DELETE", `${CONVERSATIONS}/${item.id}`);
            if (disposed)
                return;
            if (item.id === active || loading)
                reset();
            conversations = conversations.filter((entry) => entry.id !== item.id);
            historyStatus.textContent = conversations.length ? "" : "Suas conversas salvas aparecem aqui.";
            renderHistory();
        }
        catch (error) {
            toast(errorMessage(error), "error");
        }
    };
    const submit = async (text) => {
        const question = text.trim();
        if (!question || pending || loading)
            return;
        textarea.value = "";
        grow();
        const controller = new AbortController();
        pending = controller;
        send.disabled = true;
        root.classList.add("has-thread");
        thread.append(userTurn(question));
        const typing = assistantTurn(h("span", { class: "spinner", role: "status", "aria-label": "Consultando a base" }, h("i"), h("i"), h("i")));
        thread.append(typing);
        toEnd();
        try {
            await initialHistory;
            if (pending !== controller)
                return;
            if (signedIn && active === null) {
                const title = Array.from(question).slice(0, 120).join("");
                const created = (await (await request(CONVERSATIONS, { method: "POST", body: { title }, signal: controller.signal })).json());
                if (pending !== controller)
                    return;
                active = created.id;
                revision = 0;
                updateHistory(created);
            }
            const id = active;
            const body = id === null ? { question, context } : { question, conversation_id: id, conversation_revision: revision };
            const reply = (await (await request("/api/v1/assistant/ask", { method: "POST", body, signal: controller.signal })).json());
            if (pending !== controller)
                return;
            context = reply.context;
            if (id !== null) {
                revision += 1;
                const item = conversations.find((entry) => entry.id === id);
                if (item)
                    updateHistory({ ...item, turn_count: revision, updated_at: Math.floor(Date.now() / 1_000) });
            }
            typing.replaceWith(answerTurn(reply, question));
        }
        catch (error) {
            if (pending === controller && !controller.signal.aborted) {
                const code = error instanceof ApiError ? error.code : "";
                const id = active;
                typing.replaceWith(assistantTurn(h("div", { class: "answer" }, h("p", { class: "note note--error", role: "alert" }, Icon({ name: "info", size: 15 }), h("span", null, errorMessage(error) || "Falha na consulta.")), ["quota_exceeded", "free_access_limited"].includes(code) ? h("a", { href: href("conta") }, "Ver meu plano e uso") : null, code === "conversation_changed" && id !== null
                    ? Button({ label: "Reabrir conversa", size: "sm", onClick: () => void openSaved(id) })
                    : null)));
            }
        }
        finally {
            if (pending === controller) {
                pending = null;
                send.disabled = false;
                toEnd();
                textarea.focus();
            }
        }
    };
    const reset = (focus = true) => {
        pending?.abort();
        loading?.abort();
        pending = loading = null;
        context = null;
        active = null;
        revision = 0;
        send.disabled = false;
        textarea.value = "";
        grow();
        thread.replaceChildren();
        root.classList.remove("has-thread");
        historyStatus.textContent = signedIn && !conversations.length ? "Suas conversas salvas aparecem aqui." : "";
        renderHistory();
        if (focus)
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
    const root = h("div", { class: "page page--chat" }, welcome, thread, h("div", { class: "dock" }, signedIn || preview ? null : h("div", { class: "dock__bar" }, fresh), composer, h("p", { class: "dock__hint" }, "Enter envia · Shift+Enter quebra a linha")), examples);
    const workspace = signedIn || preview
        ? h("div", { class: "assistant-workspace" }, h("aside", { class: "conversation-sidebar", "aria-label": "Conversas" }, h("details", { open: true }, h("summary", null, "Conversas"), h("div", { class: "conversation-sidebar__body" }, fresh, signedIn ? history : null, signedIn
            ? historyStatus
            : h("p", { class: "conversation-status muted" }, h("a", { href: href("conta") }, "Entre com sua conta"), " para salvar e reabrir suas conversas.")))), root)
        : root;
    return {
        el: workspace,
        resume: () => textarea.focus(),
        destroy: () => {
            disposed = true;
            pending?.abort();
            loading?.abort();
            pending = loading = null;
        },
    };
}
//# sourceMappingURL=assistant-page.js.map