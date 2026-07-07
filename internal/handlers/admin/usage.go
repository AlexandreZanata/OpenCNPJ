package admin

import (
	"github.com/gofiber/fiber/v2"
)

type usagePage struct {
	LayoutData
	Rows []usageRowView
}

type usageRowView struct {
	ClientID    string
	ClientName  string
	Date        string
	Requests    int64
	CNPJLookups int64
}

// GetUsage shows recent usage across all clients.
func (h *Handler) GetUsage(c *fiber.Ctx) error {
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
	return h.html(c, "usage.html", usagePage{
		LayoutData: LayoutData{Title: "Usage", Nav: "usage", ContentTpl: "usage-content"},
		Rows:       out,
	})
}
