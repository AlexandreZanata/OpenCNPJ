package admin

import (
	"strings"

	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/audit"
	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/saas"
)

type createKeyBody struct {
	Label string `json:"label"`
}

// JSONCreateKey generates a new API key (plaintext returned once).
func (h *Handler) JSONCreateKey(c *fiber.Ctx) error {
	clientID, err := parseClientID(c.Params("id"))
	if err != nil {
		return jsonErr(c, fiber.StatusNotFound, "not_found", "client not found")
	}
	var body createKeyBody
	_ = c.BodyParser(&body)
	label := strings.TrimSpace(body.Label)
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
	_ = h.logAudit(c, adminIDFromJWT(c), audit.ActionKeyCreated, "api_client", clientID.String())
	return c.Status(fiber.StatusCreated).JSON(fiber.Map{
		"apiKey": plain,
		"prefix": saas.KeyDisplayPrefix(plain),
		"label":  label,
	})
}

// JSONRevokeKey revokes an API key.
func (h *Handler) JSONRevokeKey(c *fiber.Ctx) error {
	clientID, err := parseClientID(c.Params("id"))
	if err != nil {
		return jsonErr(c, fiber.StatusNotFound, "not_found", "client not found")
	}
	keyID, err := uuid.Parse(strings.TrimSpace(c.Params("kid")))
	if err != nil {
		return jsonErr(c, fiber.StatusNotFound, "not_found", "key not found")
	}
	_, err = h.Queries.RevokeAPIKey(c.Context(), saasdb.RevokeAPIKeyParams{
		ID: toPgUUID(keyID), ClientID: toPgUUID(clientID),
	})
	if err != nil {
		return err
	}
	_ = h.logAudit(c, adminIDFromJWT(c), audit.ActionKeyRevoked, "api_key", keyID.String())
	return c.SendStatus(fiber.StatusNoContent)
}
