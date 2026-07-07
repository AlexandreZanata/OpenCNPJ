package admin

import (
	"fmt"
	"math"
	"strconv"
	"strings"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/session"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgtype"

	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/saas"
)

const pageSize = 50

type clientRow struct {
	ID            string
	Name          string
	Email         string
	Status        string
	RateLimit     int32
	MonthlyQuota  int32
}

type clientsListPage struct {
	LayoutData
	Clients    []clientRow
	Page       int
	TotalPages int
}

type clientNewPage struct {
	LayoutData
	Error        string
	DefaultRate  int32
	DefaultQuota int32
}

type clientDetailPage struct {
	LayoutData
	Client ClientView
	Keys   []KeyView
	Usage  []UsageRow
	NewKey string
}

// ClientView is a client detail DTO for templates.
type ClientView struct {
	ID     string
	Name   string
	Email  string
	Status string
}

// KeyView is an API key row for templates.
type KeyView struct {
	ID        string
	Prefix    string
	Label     string
	CreatedAt string
	Revoked   bool
}

// UsageRow is a daily usage row for templates.
type UsageRow struct {
	Date        string
	Requests    int64
	CNPJLookups int64
}

// GetClients lists clients with pagination.
func (h *Handler) GetClients(c *fiber.Ctx) error {
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
	return h.html(c, "clients_list.html", clientsListPage{
		LayoutData: LayoutData{Title: "Clients", Nav: "clients", ContentTpl: "clients-list-content"},
		Clients:    clients,
		Page:       page,
		TotalPages: pages,
	})
}

// GetClientNew shows the create form.
func (h *Handler) GetClientNew(c *fiber.Ctx) error {
	return h.html(c, "client_new.html", clientNewPage{
		LayoutData:   LayoutData{Title: "New client", Nav: "clients", ContentTpl: "client-new-content"},
		DefaultRate:  h.DefaultRate,
		DefaultQuota: h.DefaultQuota,
	})
}

// PostClient creates a client.
func (h *Handler) PostClient(c *fiber.Ctx) error {
	name := strings.TrimSpace(c.FormValue("name"))
	email := strings.TrimSpace(c.FormValue("email"))
	rate := int32(parseIntDefault(c.FormValue("rate_limit"), int(h.DefaultRate)))
	quota := int32(parseIntDefault(c.FormValue("monthly_quota"), int(h.DefaultQuota)))
	if name == "" || email == "" {
		return h.html(c, "client_new.html", clientNewPage{
			LayoutData: LayoutData{Title: "New client", Nav: "clients", ContentTpl: "client-new-content"},
			Error:      "Name and email are required",
			DefaultRate: h.DefaultRate, DefaultQuota: h.DefaultQuota,
		})
	}
	row, err := h.Queries.InsertAPIClient(c.Context(), saasdb.InsertAPIClientParams{
		Name: name, Email: email, Status: saas.ClientStatusActive,
		MonthlyQuota: quota, RateLimitPerMin: rate,
	})
	if err != nil {
		return h.html(c, "client_new.html", clientNewPage{
			LayoutData: LayoutData{Title: "New client", Nav: "clients", ContentTpl: "client-new-content"},
			Error:      "Could not create client",
			DefaultRate: h.DefaultRate, DefaultQuota: h.DefaultQuota,
		})
	}
	id, _ := uuidFromPg(row.ID)
	return c.Redirect(fmt.Sprintf("/admin/clients/%s", id))
}

// GetClientDetail shows client, keys, and usage.
func (h *Handler) GetClientDetail(c *fiber.Ctx) error {
	id, err := parseClientID(c.Params("id"))
	if err != nil {
		return fiber.ErrNotFound
	}
	ctx := c.Context()
	client, err := h.Queries.GetClientByID(ctx, toPgUUID(id))
	if err != nil {
		return fiber.ErrNotFound
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
	page := clientDetailPage{
		LayoutData: LayoutData{Title: client.Name, Nav: "clients", ContentTpl: "client-detail-content"},
		Client: ClientView{
			ID: id.String(), Name: client.Name, Email: client.Email, Status: client.Status,
		},
		Keys:  keysFromDB(keys),
		Usage: usageFromDB(usage),
	}
	if flash := popNewKeyFlash(c, h.Session); flash != "" {
		page.NewKey = flash
	}
	return h.html(c, "client_detail.html", page)
}

// PostSuspend suspends a client.
func (h *Handler) PostSuspend(c *fiber.Ctx) error {
	id, err := parseClientID(c.Params("id"))
	if err != nil {
		return fiber.ErrNotFound
	}
	err = h.Queries.UpdateClientStatus(c.Context(), saasdb.UpdateClientStatusParams{
		ID: toPgUUID(id), Status: saas.ClientStatusSuspended,
	})
	if err != nil {
		return err
	}
	return c.Redirect(fmt.Sprintf("/admin/clients/%s", id))
}

func clientRowFromDB(r saasdb.ApiClient) clientRow {
	id, _ := uuidFromPg(r.ID)
	return clientRow{
		ID: id.String(), Name: r.Name, Email: r.Email, Status: r.Status,
		RateLimit: r.RateLimitPerMin, MonthlyQuota: r.MonthlyQuota,
	}
}

func keysFromDB(rows []saasdb.ListAPIKeysByClientRow) []KeyView {
	out := make([]KeyView, 0, len(rows))
	for _, r := range rows {
		kid, _ := uuidFromPg(r.ID)
		created := ""
		if r.CreatedAt.Valid {
			created = r.CreatedAt.Time.Format("2006-01-02")
		}
		out = append(out, KeyView{
			ID: kid.String(), Prefix: r.KeyPrefix, Label: r.Label,
			CreatedAt: created, Revoked: r.RevokedAt.Valid,
		})
	}
	return out
}

func usageFromDB(rows []saasdb.ApiUsageDaily) []UsageRow {
	out := make([]UsageRow, 0, len(rows))
	for _, r := range rows {
		d := ""
		if r.Date.Valid {
			d = r.Date.Time.Format("2006-01-02")
		}
		out = append(out, UsageRow{Date: d, Requests: r.RequestCount, CNPJLookups: r.CnpjLookupCount})
	}
	return out
}

func parseClientID(raw string) (uuid.UUID, error) {
	return uuid.Parse(strings.TrimSpace(raw))
}

func parsePage(raw string) int {
	p := parseIntDefault(raw, 1)
	if p < 1 {
		return 1
	}
	return p
}

func parseIntDefault(raw string, def int) int {
	n, err := strconv.Atoi(strings.TrimSpace(raw))
	if err != nil {
		return def
	}
	return n
}

func toPgUUID(id uuid.UUID) pgtype.UUID {
	return pgtype.UUID{Bytes: id, Valid: true}
}

func uuidFromPg(u pgtype.UUID) (uuid.UUID, error) {
	if !u.Valid {
		return uuid.Nil, fmt.Errorf("invalid uuid")
	}
	return uuid.FromBytes(u.Bytes[:])
}

func popNewKeyFlash(c *fiber.Ctx, store *session.Store) string {
	sess, err := getSess(c, store)
	if err != nil {
		return ""
	}
	v := sess.Get(sessNewKey)
	if v == nil {
		return ""
	}
	sess.Delete(sessNewKey)
	_ = sess.Save()
	s, _ := v.(string)
	return s
}

