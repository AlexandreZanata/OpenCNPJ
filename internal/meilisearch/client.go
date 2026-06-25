package meilisearch

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

const (
	IndexEmpresas         = "empresas"
	IndexEstabelecimentos = "estabelecimentos"
)

var errMeilisearchHTTP = errors.New("meilisearch request failed")

type Client struct {
	baseURL    string
	apiKey     string
	httpClient *http.Client
}

type SearchHit struct {
	ID string
}

type searchResponse struct {
	Hits []map[string]interface{} `json:"hits"`
}

func NewClient(host string, port int, apiKey string) *Client {
	return &Client{
		baseURL: fmt.Sprintf("http://%s:%d", host, port),
		apiKey:  apiKey,
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
	}
}

func (c *Client) Enabled() bool {
	return c != nil && c.baseURL != ""
}

func (c *Client) EnsureIndex(ctx context.Context, uid, primaryKey string) error {
	body := map[string]string{"uid": uid, "primaryKey": primaryKey}
	err := c.post(ctx, "/indexes", body, nil)
	if err != nil && !isIndexExistsErr(err) {
		return err
	}
	return nil
}

func isIndexExistsErr(err error) bool {
	if err == nil {
		return false
	}
	return strings.Contains(err.Error(), "already exists") || strings.Contains(err.Error(), "409")
}

func (c *Client) AddDocuments(ctx context.Context, uid string, docs []map[string]interface{}) error {
	if len(docs) == 0 {
		return nil
	}
	var resp struct {
		TaskUID int `json:"taskUid"`
	}
	return c.post(ctx, fmt.Sprintf("/indexes/%s/documents", uid), docs, &resp)
}

func (c *Client) Search(ctx context.Context, uid, query string, limit, offset int) ([]SearchHit, error) {
	body := map[string]interface{}{
		"q":      query,
		"limit":  limit,
		"offset": offset,
	}
	var out searchResponse
	if err := c.post(ctx, fmt.Sprintf("/indexes/%s/search", uid), body, &out); err != nil {
		return nil, err
	}
	hits := make([]SearchHit, 0, len(out.Hits))
	for _, hit := range out.Hits {
		id, _ := hit["id"].(string)
		if id == "" {
			if v, ok := hit["id"].(float64); ok {
				id = fmt.Sprintf("%.0f", v)
			}
		}
		if id != "" {
			hits = append(hits, SearchHit{ID: id})
		}
	}
	return hits, nil
}

func (c *Client) post(ctx context.Context, path string, body, out interface{}) error {
	return c.doJSON(ctx, http.MethodPost, path, body, out)
}

func (c *Client) patch(ctx context.Context, path string, body, out interface{}) error {
	return c.doJSON(ctx, http.MethodPatch, path, body, out)
}

// Health checks Meilisearch availability (GET /health).
func (c *Client) Health(ctx context.Context) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/health", http.NoBody)
	if err != nil {
		return fmt.Errorf("health request: %w", err)
	}
	if c.apiKey != "" {
		req.Header.Set("Authorization", "Bearer "+c.apiKey)
	}
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("health http: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		data, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("%w: health %s %s", errMeilisearchHTTP, resp.Status, string(data))
	}
	return nil
}

func (c *Client) doJSON(ctx context.Context, method, path string, body, out interface{}) error {
	payload, err := json.Marshal(body)
	if err != nil {
		return fmt.Errorf("marshal: %w", err)
	}
	req, err := http.NewRequestWithContext(ctx, method, c.baseURL+path, bytes.NewReader(payload))
	if err != nil {
		return fmt.Errorf("request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	if c.apiKey != "" {
		req.Header.Set("Authorization", "Bearer "+c.apiKey)
	}
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("http: %w", err)
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("read: %w", err)
	}
	if resp.StatusCode >= 300 {
		return fmt.Errorf("%w: %s %s", errMeilisearchHTTP, resp.Status, string(data))
	}
	if out != nil && len(data) > 0 {
		if err := json.Unmarshal(data, out); err != nil {
			return fmt.Errorf("decode: %w", err)
		}
	}
	return nil
}
