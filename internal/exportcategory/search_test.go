package exportcategory

import "testing"

func TestSearchPresetsAdvocacia(t *testing.T) {
	items := SearchPresets("advoc", 5)
	if len(items) == 0 {
		t.Fatal("expected advocacia preset match")
	}
	if items[0].Key != "advocacia" {
		t.Fatalf("expected advocacia, got %s", items[0].Key)
	}
}

func TestSearchPresetsByCNAECode(t *testing.T) {
	items := SearchPresets("6911", 5)
	found := false
	for _, item := range items {
		if item.Key == "advocacia" {
			found = true
		}
	}
	if !found {
		t.Fatal("expected advocacia when searching CNAE prefix")
	}
}
