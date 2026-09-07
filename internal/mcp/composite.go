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

// ErrDuplicateBindingName rejects a catalog builder entry that would publish
// two authorities under one canonical model-visible name.
var ErrDuplicateBindingName = errors.New("duplicate canonical binding name")

// ErrInvalidBindingSource means a source could not produce one valid catalog.
// Composite sources intentionally expose this generic boundary error instead
// of leaking the child builder's duplicate-name detail.
var ErrInvalidBindingSource = errors.New("capability binding source is invalid")

// BindingCatalogBuilder accumulates one source snapshot while preserving
// insertion order and rejecting duplicate canonical names.
type BindingCatalogBuilder struct {
	bindings []Binding
	seen     map[string]struct{}
}

// NewBindingCatalogBuilder starts an empty request-local catalog builder.
func NewBindingCatalogBuilder() *BindingCatalogBuilder {
	return &BindingCatalogBuilder{seen: make(map[string]struct{})}
}

// Add appends one binding unless its canonical name already exists.
func (builder *BindingCatalogBuilder) Add(binding Binding) error {
	if builder == nil {
		return ErrInvalidBindingSource
	}
	if _, exists := builder.seen[binding.Name]; exists {
		return ErrDuplicateBindingName
	}
	builder.seen[binding.Name] = struct{}{}
	builder.bindings = append(builder.bindings, binding)
	return nil
}

// Build returns the stable insertion-ordered snapshot.
func (builder *BindingCatalogBuilder) Build() []Binding {
	if builder == nil {
		return nil
	}
	return append([]Binding(nil), builder.bindings...)
}

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
	builder := NewBindingCatalogBuilder()
	var result BindingCatalogResult
	for _, child := range source.sources {
		current, err := child.Catalog(ctx)
		if err != nil {
			return BindingCatalogResult{}, err
		}
		for _, binding := range current.Bindings {
			if err := builder.Add(binding); err != nil {
				return BindingCatalogResult{}, ErrInvalidBindingSource
			}
		}
		result.AvailabilityNotices = append(result.AvailabilityNotices, current.AvailabilityNotices...)
	}
	result.Bindings = builder.Build()
	return result, nil
}
