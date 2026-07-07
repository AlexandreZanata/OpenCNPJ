package middleware

import (
	"errors"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/saas"
)

// APIKey validates X-API-Key, enforces rate limits and quota, records usage.
func APIKey(deps *saas.Deps) fiber.Handler {
	if deps == nil || deps.Auth == nil {
		return func(c *fiber.Ctx) error { return c.Next() }
	}
	return func(c *fiber.Ctx) error {
		raw := c.Get(saas.HeaderAPIKey)
		if raw == "" {
			return apiKeyError(c, fiber.StatusUnauthorized, "missing_api_key", saas.ErrMissingKey)
		}
		client, err := deps.Auth.Authenticate(c.Context(), raw)
		if err != nil {
			return mapAuthError(c, err)
		}
		if deps.RateLimit != nil {
			ok, rlErr := deps.RateLimit.Allow(c.Context(), client.ClientID, client.RateLimitPerMin)
			if rlErr != nil {
				return apiKeyError(c, fiber.StatusInternalServerError, "rate_limit_error", rlErr)
			}
			if !ok {
				return apiKeyError(c, fiber.StatusTooManyRequests, "rate_limit_exceeded", saas.ErrRateLimited)
			}
		}
		if client.MonthlyQuota > 0 && deps.Usage != nil {
			count, qErr := deps.Usage.MonthCount(c.Context(), client.ClientID)
			if qErr != nil {
				return apiKeyError(c, fiber.StatusInternalServerError, "quota_check_error", qErr)
			}
			if count >= int64(client.MonthlyQuota) {
				return apiKeyError(c, fiber.StatusTooManyRequests, "quota_exceeded", saas.ErrQuotaExceeded)
			}
		}
		if deps.Usage != nil {
			deps.Usage.RecordRequest(client.ClientID)
		}
		c.Locals(saas.ClientContextKey, client)
		return c.Next()
	}
}

// ClientFromCtx returns the authenticated client attached by APIKey middleware.
func ClientFromCtx(c *fiber.Ctx) (saas.AuthenticatedClient, bool) {
	v := c.Locals(saas.ClientContextKey)
	if v == nil {
		return saas.AuthenticatedClient{}, false
	}
	client, ok := v.(saas.AuthenticatedClient)
	return client, ok
}

func mapAuthError(c *fiber.Ctx, err error) error {
	switch {
	case errors.Is(err, saas.ErrMissingKey), errors.Is(err, saas.ErrInvalidKey):
		return apiKeyError(c, fiber.StatusUnauthorized, "invalid_api_key", err)
	case errors.Is(err, saas.ErrExpiredKey):
		return apiKeyError(c, fiber.StatusUnauthorized, "expired_api_key", err)
	case errors.Is(err, saas.ErrSuspendedClient):
		return apiKeyError(c, fiber.StatusForbidden, "client_suspended", err)
	default:
		return apiKeyError(c, fiber.StatusInternalServerError, "auth_error", err)
	}
}

func apiKeyError(c *fiber.Ctx, code int, errCode string, err error) error {
	return c.Status(code).JSON(fiber.Map{
		"error":   errCode,
		"message": err.Error(),
		"code":    code,
	})
}

// RecordCNPJLookup increments CNPJ usage when a handler serves a lookup.
func RecordCNPJLookup(c *fiber.Ctx, usage saas.UsageRecorder) {
	if usage == nil {
		return
	}
	client, ok := ClientFromCtx(c)
	if !ok {
		return
	}
	usage.RecordCNPJLookup(client.ClientID)
}
