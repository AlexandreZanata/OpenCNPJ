package repository

// PageMeta carries pagination metadata for search results.
type PageMeta struct {
	Total      int64
	HasMore    bool
	NextCursor string
}
