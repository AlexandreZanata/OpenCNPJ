package downloader

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"sort"
	"strings"
	"time"
)

const (
	DefaultBaseURL = "https://arquivos.receitafederal.gov.br/public.php/webdav"
	// #nosec G101 -- public read-only Nextcloud share token; not a secret credential.
	DefaultShareToken = "YggdBLfdninEJX9"
)

var (
	monthDirPattern = regexp.MustCompile(`(\d{4}-\d{2})/?$`)
	hrefPattern     = regexp.MustCompile(`<(?:d:)?href>([^<]+)</(?:d:)?href>`)
)

// Client talks to the Receita Federal public Nextcloud share via WebDAV.
type Client struct {
	baseURL    string
	shareToken string
	httpClient *http.Client
}

func NewClient(baseURL, shareToken string, timeout time.Duration) *Client {
	if baseURL == "" {
		baseURL = DefaultBaseURL
	}
	if shareToken == "" {
		shareToken = DefaultShareToken
	}
	if timeout <= 0 {
		timeout = 30 * time.Minute
	}

	return &Client{
		baseURL:    strings.TrimRight(baseURL, "/"),
		shareToken: shareToken,
		httpClient: &http.Client{Timeout: timeout},
	}
}

func (c *Client) ListMonthDirectories(ctx context.Context) ([]string, error) {
	body, err := c.propfind(ctx, "")
	if err != nil {
		return nil, err
	}

	var dirs []string
	for _, href := range parseHrefs(body) {
		if match := monthDirPattern.FindStringSubmatch(href); match != nil {
			dirs = append(dirs, match[1])
		}
	}
	if len(dirs) == 0 {
		return nil, ErrNoMonthlyDirs
	}

	sort.Strings(dirs)
	return dirs, nil
}

func (c *Client) ListZipFiles(ctx context.Context, month string) ([]string, error) {
	body, err := c.propfind(ctx, month)
	if err != nil {
		return nil, err
	}

	hrefs := parseHrefs(body)
	seen := make(map[string]struct{}, len(hrefs))
	files := make([]string, 0, len(hrefs))
	for _, href := range hrefs {
		name := extractZipName(href)
		if name == "" {
			continue
		}
		if _, ok := seen[name]; ok {
			continue
		}
		seen[name] = struct{}{}
		files = append(files, name)
	}
	if len(files) == 0 {
		return nil, fmt.Errorf("%w: %s", ErrNoZipFiles, month)
	}
	sort.Strings(files)
	return files, nil
}

func (c *Client) Download(ctx context.Context, month, filename string) (io.ReadCloser, int64, error) {
	url := fmt.Sprintf("%s/%s/%s", c.baseURL, month, filename)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, http.NoBody)
	if err != nil {
		return nil, 0, err
	}
	req.SetBasicAuth(c.shareToken, "")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, 0, fmt.Errorf("download %s: %w", filename, err)
	}
	if resp.StatusCode != http.StatusOK {
		_ = resp.Body.Close()
		return nil, 0, fmt.Errorf("download %s: %w: %s", filename, ErrUnexpectedHTTPStatus, resp.Status)
	}

	return resp.Body, resp.ContentLength, nil
}

func (c *Client) propfind(ctx context.Context, path string) ([]byte, error) {
	url := c.baseURL + "/"
	if path != "" {
		url = fmt.Sprintf("%s/%s/", c.baseURL, strings.Trim(path, "/"))
	}

	req, err := http.NewRequestWithContext(ctx, "PROPFIND", url, http.NoBody)
	if err != nil {
		return nil, err
	}
	req.SetBasicAuth(c.shareToken, "")
	req.Header.Set("Depth", "1")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("propfind %s: %w", path, err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode != http.StatusMultiStatus && resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("propfind %s: %w: %s", path, ErrUnexpectedHTTPStatus, resp.Status)
	}
	return body, nil
}

func parseHrefs(body []byte) []string {
	matches := hrefPattern.FindAllStringSubmatch(string(body), -1)
	hrefs := make([]string, 0, len(matches))
	for _, m := range matches {
		if len(m) > 1 && m[1] != "" {
			hrefs = append(hrefs, m[1])
		}
	}
	return hrefs
}

func extractZipName(href string) string {
	parts := strings.Split(href, "/")
	for i := len(parts) - 1; i >= 0; i-- {
		if strings.HasSuffix(strings.ToLower(parts[i]), ".zip") {
			return parts[i]
		}
	}
	return ""
}
