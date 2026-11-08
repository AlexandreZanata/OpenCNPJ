import { h } from "../../core/dom.js";
import { routeHref } from "../../core/router.js";
import { Dropdown } from "../../ui/dropdown.js";
import { Icon } from "../../ui/icon.js";
/** Floating top-right controls: notifications and plan menu. */
export function Topbar({ planName, hasNotifications, onNavigate }) {
    const bell = h("button", { class: "icon-button", type: "button", "aria-label": hasNotifications ? "Notificações (há novas)" : "Notificações" }, Icon({ name: "bell", size: 22, strokeWidth: 1.8 }), hasNotifications && h("span", { class: "icon-button__dot", "aria-hidden": "true" }));
    const plan = Dropdown({
        trigger: { label: planName, icon: "crown", trailingIcon: "chevron-down", class: "btn--plan" },
        items: [
            { id: "plan", label: "Gerenciar plano", onSelect: () => onNavigate(routeHref("configuracoes")) },
            { id: "usage", label: "Uso e limites", onSelect: () => onNavigate(routeHref("api")) },
        ],
    });
    return h("div", { class: "topbar" }, bell, plan.el);
}
//# sourceMappingURL=topbar.js.map