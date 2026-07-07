package admin

import (
	"fmt"
	"strings"

	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/audit"
	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/saas"
)

// PostCreateKey generates a new API key and flashes plaintext once.
func (h *Handler) PostCreateKey(c *fiber.Ctx) error {
	clientID, err := parseClientID(c.Params("id"))
	if err != nil {
		return fiber.ErrNotFound
	}
	label := strings.TrimSpace(c.FormValue("label"))
	if label == "" {
		label = "production"
	}
	plain, err := saas.GenerateKey()
	if err != nil {
		return err
	}
	_, err = h.Queries.InsertAPIKey(c.Context(), saasdb.InsertAPIKeyParams{
		ClientID:  toPgUUID(clientID),
		KeyPrefix: saas.KeyDisplayPrefix(plain),
		KeyHash:   saas.HashKey(plain),
		Label:     label,
	})
	if err != nil {
		return err
	}
	_ = h.logAudit(c, h.adminIDFromCtx(c), audit.ActionKeyCreated, "api_client", clientID.String(), nil)
	sess, err := getSess(c, h.Session)
	if err != nil {
		return err
	}
	sess.Set(sessNewKey, plain)
	if err := sess.Save(); err != nil {
		return err
	}
	return c.Redirect(fmt.Sprintf("/admin/clients/%s", clientID))
}

// PostRevokeKey revokes an API key.
func (h *Handler) PostRevokeKey(c *fiber.Ctx) error {
	clientID, err := parseClientID(c.Params("id"))
	if err != nil {
		return fiber.ErrNotFound
	}
	keyID, err := uuid.Parse(strings.TrimSpace(c.Params("kid")))
	if err != nil {
		return fiber.ErrNotFound
	}
	_, err = h.Queries.RevokeAPIKey(c.Context(), saasdb.RevokeAPIKeyParams{
		ID: toPgUUID(keyID), ClientID: toPgUUID(clientID),
	})
	if err != nil {
		return err
	}
	_ = h.logAudit(c, h.adminIDFromCtx(c), audit.ActionKeyRevoked, "api_key", keyID.String(), nil)
	return c.Redirect(fmt.Sprintf("/admin/clients/%s", clientID))
}
