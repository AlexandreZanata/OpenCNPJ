package exportcategory

import "testing"

func TestFindAdvocacia(t *testing.T) {
	cat, ok := Find("advocacia")
	if !ok {
		t.Fatal("expected advocacia category")
	}
	if len(cat.CNAECodes) < 2 {
		t.Fatalf("expected CNAE codes, got %v", cat.CNAECodes)
	}
}

func TestMatchSQLBuildsOrClause(t *testing.T) {
	cat, _ := Find("advocacia")
	args := make([]any, 0)
	pos := 1
	clause := MatchSQL(cat, &pos, &args)
	if clause == "" || clause == "1=0" {
		t.Fatalf("unexpected clause: %s", clause)
	}
	if len(args) == 0 {
		t.Fatal("expected args")
	}
}

func TestFindUnknown(t *testing.T) {
	if _, ok := Find("unknown-category"); ok {
		t.Fatal("expected missing category")
	}
}
