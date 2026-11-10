/** Sign-in with the login key, account creation and key recovery by e-mail. */

import { errorMessage, login, sendJson } from "../core/api.js";
import { h } from "../core/dom.js";
import { session } from "../core/session.js";
import { ThemeToggle } from "../layout/theme-toggle.js";
import { Logo } from "../layout/logo.js";
import { Button, Field, Input, busy } from "../ui/kit.js";
import type { Page } from "./page.js";

type Mode = "entrar" | "criar" | "recuperar";

export function LoginPage(initial: Mode = "entrar", standalone = false): Page {
  const card = h("div", { class: "auth__card card" });
  const registration = session().platform?.registration !== "closed";

  const message = (text: string, tone: "error" | "ok"): HTMLElement => h("p", { class: `note note--${tone}`, role: tone === "error" ? "alert" : "status" }, text);

  const render = (mode: Mode, notice?: HTMLElement): void => {
    const feedback = h("div", { class: "auth__feedback" }, notice ?? null);
    const fail = (error: unknown): void => feedback.replaceChildren(message(errorMessage(error) || "Não consegui concluir. Tente de novo.", "error"));
    const link = (label: string, to: Mode): HTMLElement => h("button", { class: "link", type: "button", on: { click: () => render(to) } }, label);

    if (mode === "entrar") {
      const key = Input({ type: "password", placeholder: "ocnpj_…", autocomplete: "current-password", required: true, class: "input--lg" });
      const submit = Button({ label: "Entrar", variant: "primary", type: "submit", class: "btn--block" });
      card.replaceChildren(
        h("h1", null, "Entrar"),
        h("p", { class: "muted" }, "Use a chave de acesso que você recebeu por e-mail. A mesma chave serve para a API."),
        h(
          "form",
          {
            class: "auth__form",
            on: {
              submit: (event) => {
                event.preventDefault();
                void busy(submit, async () => {
                  try {
                    await login(key.value);
                  } catch (error) {
                    fail(error);
                  }
                });
              },
            },
          },
          Field({ label: "Chave de acesso", control: key }),
          feedback,
          submit,
        ),
        h("div", { class: "auth__links" }, registration ? link("Criar uma conta", "criar") : null, link("Perdi minha chave", "recuperar")),
      );
      key.focus();
      return;
    }

    if (mode === "criar") {
      const username = Input({ placeholder: "seu.nome", autocomplete: "username", required: true, maxlength: 32 });
      const email = Input({ type: "email", placeholder: "voce@empresa.com.br", autocomplete: "email", required: true });
      const submit = Button({ label: "Criar conta", variant: "primary", type: "submit", class: "btn--block" });
      card.replaceChildren(
        h("h1", null, "Criar conta"),
        h("p", { class: "muted" }, "Sua conta começa no plano gratuito. Enviamos a chave de acesso para o seu e-mail."),
        h(
          "form",
          {
            class: "auth__form",
            on: {
              submit: (event) => {
                event.preventDefault();
                void busy(submit, async () => {
                  try {
                    await sendJson("POST", "/api/v1/auth/register", { username: username.value.trim().toLowerCase(), email: email.value.trim() });
                    render("entrar", message("Pronto. Se o e-mail estiver correto, a chave chega em instantes.", "ok"));
                  } catch (error) {
                    fail(error);
                  }
                });
              },
            },
          },
          Field({ label: "Nome de usuário", control: username, hint: "3 a 32 caracteres: letras minúsculas, números, ponto, hífen ou sublinhado." }),
          Field({ label: "E-mail", control: email }),
          feedback,
          submit,
        ),
        h("div", { class: "auth__links" }, link("Já tenho uma chave", "entrar")),
      );
      username.focus();
      return;
    }

    const email = Input({ type: "email", placeholder: "voce@empresa.com.br", autocomplete: "email", required: true });
    const submit = Button({ label: "Enviar link", variant: "primary", type: "submit", class: "btn--block" });
    card.replaceChildren(
      h("h1", null, "Recuperar a chave"),
      h("p", { class: "muted" }, "Enviamos um link válido por 30 minutos. Ao confirmar, a chave antiga deixa de funcionar e uma nova é emitida."),
      h(
        "form",
        {
          class: "auth__form",
          on: {
            submit: (event) => {
              event.preventDefault();
              void busy(submit, async () => {
                try {
                  await sendJson("POST", "/api/v1/auth/recover", { email: email.value.trim() });
                  render("entrar", message("Se houver uma conta com esse e-mail, o link já foi enviado.", "ok"));
                } catch (error) {
                  fail(error);
                }
              });
            },
          },
        },
        Field({ label: "E-mail da conta", control: email }),
        feedback,
        submit,
      ),
      h("div", { class: "auth__links" }, link("Voltar para entrar", "entrar")),
    );
    email.focus();
  };

  render(initial === "criar" && !registration ? "entrar" : initial);
  return {
    el: h(
      "div",
      { class: "auth" },
      standalone ? h("div", { class: "auth__theme" }, ThemeToggle()) : null,
      h("div", { class: "auth__brand" }, Logo(), h("p", { class: "muted" }, "Pesquisa comercial sobre os dados oficiais do CNPJ.")),
      card,
    ),
  };
}
