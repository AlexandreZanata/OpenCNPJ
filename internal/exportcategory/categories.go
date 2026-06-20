package exportcategory

import (
	"fmt"
	"strings"
)

// Category maps a business segment to CNAE codes and description keywords.
type Category struct {
	Key         string
	Label       string
	Description string
	CNAECodes   []string
	Keywords    []string
}

var catalog = []Category{
	{
		Key:         "advocacia",
		Label:       "Legal / Advocacia",
		Description: "Law firms and legal consulting",
		CNAECodes:   []string{"6911701", "6911702"},
		Keywords:    []string{"advoc", "juridic", "advogad"},
	},
	{
		Key:         "contabilidade",
		Label:       "Accounting",
		Description: "Accounting, bookkeeping, and tax consulting",
		CNAECodes:   []string{"6920601", "6920602"},
		Keywords:    []string{"contab", "fiscal"},
	},
	{
		Key:         "medicina",
		Label:       "Healthcare",
		Description: "Clinics, doctors, and healthcare services",
		CNAECodes:   []string{"8630501", "8630502", "8630503", "8610101", "8610102"},
		Keywords:    []string{"clinica", "medico", "odontolog", "hospital"},
	},
	{
		Key:         "restaurante",
		Label:       "Food & Restaurants",
		Description: "Restaurants, bars, and food services",
		CNAECodes:   []string{"5611201", "5611203", "5611204", "5611205"},
		Keywords:    []string{"restaur", "lanchon", "bar ", "pizzaria"},
	},
	{
		Key:         "tecnologia",
		Label:       "Technology",
		Description: "Software development and IT services",
		CNAECodes:   []string{"6201501", "6202300", "6203100", "6204000", "6311900"},
		Keywords:    []string{"tecnolog", "software", "informatica", "sistemas"},
	},
	{
		Key:         "imobiliaria",
		Label:       "Real Estate",
		Description: "Real estate agencies and property management",
		CNAECodes:   []string{"6810201", "6810202", "6821801", "6822600"},
		Keywords:    []string{"imobili", "corretor"},
	},
	{
		Key:         "educacao",
		Label:       "Education",
		Description: "Schools and training centers",
		CNAECodes:   []string{"8513900", "8511200", "8599604", "8599699"},
		Keywords:    []string{"escola", "ensino", "curso", "colegio"},
	},
	{
		Key:         "beleza",
		Label:       "Beauty & Aesthetics",
		Description: "Salons, barbershops, and aesthetics",
		CNAECodes:   []string{"9602501", "9602502", "9609202", "9609207"},
		Keywords:    []string{"cabeleir", "estetica", "beleza", "barbear"},
	},
}

// List returns all export categories for the API.
func List() []Category {
	out := make([]Category, len(catalog))
	copy(out, catalog)
	return out
}

// Find returns a category by key (case-insensitive).
func Find(key string) (Category, bool) {
	normalized := strings.ToLower(strings.TrimSpace(key))
	for _, item := range catalog {
		if item.Key == normalized {
			return item, true
		}
	}
	return Category{}, false
}

// MatchSQL builds OR clauses for CNAE codes and CNAE description keywords.
func MatchSQL(category Category, argPos *int, args *[]any) string {
	parts := make([]string, 0, len(category.CNAECodes)+len(category.Keywords))

	for _, code := range category.CNAECodes {
		parts = append(parts, buildPlaceholder("e.cnae_fiscal_principal =", argPos, args, code))
	}
	for _, keyword := range category.Keywords {
		parts = append(parts, buildPlaceholder("c.descricao ILIKE", argPos, args, "%"+keyword+"%"))
	}

	if len(parts) == 0 {
		return "1=0"
	}
	return "(" + strings.Join(parts, " OR ") + ")"
}

func buildPlaceholder(expr string, argPos *int, args *[]any, value any) string {
	clause := fmt.Sprintf("%s $%d", expr, *argPos)
	*argPos++
	*args = append(*args, value)
	return clause
}
