package mcp

import (
	"context"
	"errors"
)

// BindingAvailabilityStatus is the stable reason a binding is absent.
type BindingAvailabilityStatus string

const (
	BindingUnavailable            BindingAvailabilityStatus = "unavailable"
	BindingAuthenticationRequired BindingAvailabilityStatus = "authentication_required"
)

// BindingAvailabilityNotice explains why one source binding is unavailable.
type BindingAvailabilityNotice struct {
	Capability *string
	Status     BindingAvailabilityStatus
}

// BindingCatalogResult is one source snapshot and its availability notices.
type BindingCatalogResult struct {
	Bindings            []Binding
	AvailabilityNotices []BindingAvailabilityNotice
}

// BindingSource supplies a request-local binding snapshot.
type BindingSource interface {
	Catalog(context.Context) (BindingCatalogResult, error)
}

// BindingSourceFunc adapts one function to BindingSource.
type BindingSourceFunc func(context.Context) (BindingCatalogResult, error)

func (source BindingSourceFunc) Catalog(ctx context.Context) (BindingCatalogResult, error) {
	return source(ctx)
}

// ErrDuplicateBindingName rejects a composed catalog that would publish two
// authorities under one canonical model-visible name.
var ErrDuplicateBindingName = errors.New("duplicate canonical binding name")

// CompositeBindingSource loads configured sources in order and preserves that
// order for both bindings and availability notices.
type CompositeBindingSource struct {
	sources []BindingSource
}

// NewCompositeBindingSource constructs one deterministic source composition.
func NewCompositeBindingSource(sources ...BindingSource) *CompositeBindingSource {
	return &CompositeBindingSource{sources: append([]BindingSource(nil), sources...)}
}

// Catalog returns the complete composed snapshot or rejects it atomically.
func (source *CompositeBindingSource) Catalog(ctx context.Context) (BindingCatalogResult, error) {
	var result BindingCatalogResult
	seen := make(map[string]struct{})
	for _, child := range source.sources {
		current, err := child.Catalog(ctx)
		if err != nil {
			return BindingCatalogResult{}, err
		}
		for _, binding := range current.Bindings {
			if _, exists := seen[binding.Name]; exists {
				return BindingCatalogResult{}, ErrDuplicateBindingName
			}
			seen[binding.Name] = struct{}{}
			result.Bindings = append(result.Bindings, binding)
		}
		result.AvailabilityNotices = append(result.AvailabilityNotices, current.AvailabilityNotices...)
	}
	return result, nil
}
