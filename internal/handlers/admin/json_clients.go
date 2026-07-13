package admin

import (
	"math"
	"strings"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/adminauth/audit"
	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/saas"
)

type createClientBody struct {
	Name            string `json:"name"`
	Email           string `json:"email"`
	RateLimitPerMin int32  `json:"rate_limit_per_min"`
	MonthlyQuota    int32  `json:"monthly_quota"`
}

// JSONDashboard returns summary stats for the SPA admin panel.
func (h *Handler) JSONDashboard(c *fiber.Ctx) error {
	ctx := c.Context()
	clients, err := h.Queries.CountAPIClients(ctx)
	if err != nil {
		return err
	}
	today, err := h.Queries.SumUsageRequestsToday(ctx)
	if err != nil {
		return err
	}
	return c.JSON(fiber.Map{"totalClients": clients, "requestsToday": today})
}

// JSONListClients returns paginated API clients.
func (h *Handler) JSONListClients(c *fiber.Ctx) error {
	page := parsePage(c.Query("page"))
	offset := int32((page - 1) * pageSize)
	rows, err := h.Queries.ListAPIClients(c.Context(), saasdb.ListAPIClientsParams{
		Limit: pageSize, Offset: offset,
	})
	if err != nil {
		return err
	}
	total, err := h.Queries.CountAPIClients(c.Context())
	if err != nil {
		return err
	}
	clients := make([]clientRow, 0, len(rows))
	for _, r := range rows {
		clients = append(clients, clientRowFromDB(r))
	}
	pages := int(math.Ceil(float64(total) / float64(pageSize)))
	if pages < 1 {
		pages = 1
	}
	return c.JSON(fiber.Map{
		"clients": clients, "page": page, "totalPages": pages, "total": total,
	})
}

// JSONGetClient returns client detail with keys and usage.
func (h *Handler) JSONGetClient(c *fiber.Ctx) error {
	id, err := parseClientID(c.Params("id"))
	if err != nil {
		return jsonErr(c, fiber.StatusNotFound, "not_found", "client not found")
	}
	ctx := c.Context()
	client, err := h.Queries.GetClientByID(ctx, toPgUUID(id))
	if err != nil {
		return jsonErr(c, fiber.StatusNotFound, "not_found", "client not found")
	}
	keys, err := h.Queries.ListAPIKeysByClient(ctx, toPgUUID(id))
	if err != nil {
		return err
	}
	usage, err := h.Queries.ListUsageByClient(ctx, saasdb.ListUsageByClientParams{
		ClientID: toPgUUID(id), Limit: 30,
	})
	if err != nil {
		return err
	}
	return c.JSON(fiber.Map{
		"client": ClientView{ID: id.String(), Name: client.Name, Email: client.Email, Status: client.Status},
		"keys":   keysFromDB(keys),
		"usage":  usageFromDB(usage),
	})
}

// JSONCreateClient creates an API client.
func (h *Handler) JSONCreateClient(c *fiber.Ctx) error {
	var body createClientBody
	if err := c.BodyParser(&body); err != nil {
		return jsonErr(c, fiber.StatusBadRequest, "invalid_json", "invalid request body")
	}
	name := strings.TrimSpace(body.Name)
	email := strings.TrimSpace(body.Email)
	if name == "" || email == "" {
		return jsonErr(c, fiber.StatusBadRequest, "validation_error", "name and email are required")
	}
	rate := body.RateLimitPerMin
	if rate <= 0 {
		rate = h.DefaultRate
	}
	quota := body.MonthlyQuota
	if quota < 0 {
		quota = h.DefaultQuota
	}
	row, err := h.Queries.InsertAPIClient(c.Context(), saasdb.InsertAPIClientParams{
		Name: name, Email: email, Status: saas.ClientStatusActive,
		MonthlyQuota: quota, RateLimitPerMin: rate,
	})
	if err != nil {
		return jsonErr(c, fiber.StatusInternalServerError, "create_failed", "could not create client")
	}
	cid, _ := uuidFromPg(row.ID)
	_ = h.logAudit(c, adminIDFromJWT(c), audit.ActionClientCreated, "api_client", cid.String())
	return c.Status(fiber.StatusCreated).JSON(clientRowFromDB(row))
}

// JSONSuspendClient suspends a client.
func (h *Handler) JSONSuspendClient(c *fiber.Ctx) error {
	id, err := parseClientID(c.Params("id"))
	if err != nil {
		return jsonErr(c, fiber.StatusNotFound, "not_found", "client not found")
	}
	err = h.Queries.UpdateClientStatus(c.Context(), saasdb.UpdateClientStatusParams{
		ID: toPgUUID(id), Status: saas.ClientStatusSuspended,
	})
	if err != nil {
		return err
	}
	_ = h.logAudit(c, adminIDFromJWT(c), audit.ActionClientSuspended, "api_client", id.String())
	return c.SendStatus(fiber.StatusNoContent)
}
