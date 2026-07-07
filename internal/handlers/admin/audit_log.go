package admin

import (
	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"
)

func (h *Handler) logAudit(
	c *fiber.Ctx,
	adminID uuid.UUID,
	action, resourceType, resourceID string,
) error {
	if h.Audit == nil {
		return nil
	}
	return h.Audit.Log(c.Context(), adminID, action, resourceType, resourceID, nil)
}

func (h *Handler) adminIDFromCtx(c *fiber.Ctx) uuid.UUID {
	v := c.Locals("adminID")
	if v == nil {
		return uuid.Nil
	}
	id, ok := v.(uuid.UUID)
	if !ok {
		return uuid.Nil
	}
	return id
}
