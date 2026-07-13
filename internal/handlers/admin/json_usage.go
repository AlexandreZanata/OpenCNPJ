package admin

import "github.com/gofiber/fiber/v2"

// JSONUsage returns recent usage across all clients.
func (h *Handler) JSONUsage(c *fiber.Ctx) error {
	rows, err := h.Queries.ListRecentUsage(c.Context(), 100)
	if err != nil {
		return err
	}
	out := make([]usageRowView, 0, len(rows))
	for _, r := range rows {
		cid, _ := uuidFromPg(r.ClientID)
		d := ""
		if r.Date.Valid {
			d = r.Date.Time.Format("2006-01-02")
		}
		out = append(out, usageRowView{
			ClientID: cid.String(), ClientName: r.ClientName, Date: d,
			Requests: r.RequestCount, CNPJLookups: r.CnpjLookupCount,
		})
	}
	return c.JSON(fiber.Map{"rows": out})
}
