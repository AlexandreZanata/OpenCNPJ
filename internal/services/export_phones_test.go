package services

import "testing"

func TestListExportCategories(t *testing.T) {
	svc := NewExportService()
	items := svc.ListExportCategories()
	if len(items) == 0 {
		t.Fatal("expected export categories")
	}
	if items[0].Key == "" || items[0].Label == "" {
		t.Fatalf("invalid category: %+v", items[0])
	}
}
