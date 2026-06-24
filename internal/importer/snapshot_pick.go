package importer

import (
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

var (
	rfbDateRe  = regexp.MustCompile(`\.D(\d{5})(?:\.|$)`)
	rfbShardRe = regexp.MustCompile(`(K03200Y\d+)`)
)

func rfbSnapshotDate(name string) int {
	upper := strings.ToUpper(name)
	match := rfbDateRe.FindStringSubmatch(upper)
	if len(match) < 2 {
		return 0
	}
	date, _ := strconv.Atoi(match[1])
	return date
}

func rfbPartitionKey(name string) string {
	upper := strings.ToUpper(name)
	if match := rfbShardRe.FindStringSubmatch(upper); len(match) >= 2 {
		return match[1]
	}
	if i := strings.Index(upper, ".D"); i > 0 {
		return upper[:i]
	}
	return upper
}

func pickLatestByPartition(paths []string) []string {
	best := make(map[string]string, len(paths))
	bestDate := make(map[string]int, len(paths))
	for _, path := range paths {
		name := filepath.Base(path)
		key := rfbPartitionKey(name)
		date := rfbSnapshotDate(name)
		if prev, ok := bestDate[key]; !ok || date > prev {
			best[key] = path
			bestDate[key] = date
		}
	}
	out := make([]string, 0, len(best))
	for _, path := range best {
		out = append(out, path)
	}
	sort.Strings(out)
	return out
}

func pickLatestPath(current, candidate string) string {
	if current == "" {
		return candidate
	}
	if rfbSnapshotDate(filepath.Base(candidate)) > rfbSnapshotDate(filepath.Base(current)) {
		return candidate
	}
	return current
}
