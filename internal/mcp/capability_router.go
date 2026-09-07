package mcp

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
)

// InvokerKey identifies one server-owned capability invoker registration.
// Provider arguments cannot override this value.
type InvokerKey string

// CapabilityInvocation is the exact target and argument payload selected from
// one immutable binding snapshot.
type CapabilityInvocation struct {
	Operation             string
	OperationToken        string
	Arguments             any
	ReviewedAuthorization *ReviewedAuthorization
}

// CapabilityOutput is the model-visible result of one completed invocation.
// PersistedOutput is an optional richer source for durable output views.
type CapabilityOutput struct {
	Success            bool
	Payload            any
	Failure            *CapabilityFailure
	PersistedOutput    any
	PersistedOutputSet bool
}

// CapabilityFailure carries the server-owned recovery fields for a completed
// tool-declared failure. Transport and control-plane failures use an error.
type CapabilityFailure struct {
	Kind     string
	Recovery string
}

// WithPersistedOutputSource supplies a durable output view without changing
// the model-visible payload.
func (output CapabilityOutput) WithPersistedOutputSource(value any) CapabilityOutput {
	output.PersistedOutput = value
	output.PersistedOutputSet = true
	return output
}

func (output CapabilityOutput) materializeFailure() CapabilityOutput {
	if output.Failure == nil {
		return output
	}
	output.Payload = cloneCapabilityValue(output.Payload)
	fields, ok := output.Payload.(map[string]any)
	if !ok {
		output.Payload = map[string]any{"result": output.Payload}
		fields = output.Payload.(map[string]any)
	}
	fields["failure_kind"] = output.Failure.Kind
	fields["recovery"] = output.Failure.Recovery
	return output
}

// CapabilityInvoker executes one target selected by the registry router.
type CapabilityInvoker interface {
	Invoke(context.Context, CapabilityInvocation) (CapabilityOutput, error)
}

// CapabilityInvokerFunc adapts a function to CapabilityInvoker.
type CapabilityInvokerFunc func(context.Context, CapabilityInvocation) (CapabilityOutput, error)

func (fn CapabilityInvokerFunc) Invoke(ctx context.Context, invocation CapabilityInvocation) (CapabilityOutput, error) {
	return fn(ctx, invocation)
}

// CapabilityInvokerRegistration binds one opaque key to one implementation.
type CapabilityInvokerRegistration struct {
	Key     InvokerKey
	Invoker CapabilityInvoker
}

// ErrDuplicateCapabilityInvoker means that one opaque invoker key was
// registered more than once.
var ErrDuplicateCapabilityInvoker = errors.New("duplicate capability invoker registration")

// The router errors are stable control-plane categories. Their messages are
// safe to expose in the model payload and contain no provider diagnostics.
var (
	ErrCapabilityUnknownInvoker         = errors.New("capability invoker is unavailable")
	ErrCapabilityUnknownOperation       = errors.New("capability operation is unavailable")
	ErrCapabilityInvalidArguments       = errors.New("capability arguments are invalid")
	ErrCapabilityDenied                 = errors.New("capability invocation was denied")
	ErrCapabilityUnavailable            = errors.New("capability is unavailable")
	ErrCapabilityAuthenticationRequired = errors.New("capability authentication is required")
	ErrCapabilityFailed                 = errors.New("capability invocation failed")
	ErrCapabilityOutcomeUncertain       = errors.New("capability outcome is uncertain")
)

// CapabilityDispatch is a completed invocation with model and durable views.
type CapabilityDispatch struct {
	Output    CapabilityOutput
	Persisted PersistedBindingViews
}

// CapabilityDispatchFailure is a control-plane failure after the binding's
// persistence policy has been applied.
type CapabilityDispatchFailure struct {
	Error     error
	Persisted PersistedBindingViews
}

// CapabilityRegistryRouter resolves immutable binding snapshots through a
// strict opaque-invoker registry.
type CapabilityRegistryRouter struct {
	invokers map[InvokerKey]CapabilityInvoker
}

// NewCapabilityRegistryRouter rejects duplicate invoker registrations.
func NewCapabilityRegistryRouter(registrations ...CapabilityInvokerRegistration) (*CapabilityRegistryRouter, error) {
	invokers := make(map[InvokerKey]CapabilityInvoker, len(registrations))
	for _, registration := range registrations {
		if registration.Invoker == nil {
			return nil, ErrDuplicateCapabilityInvoker
		}
		if _, exists := invokers[registration.Key]; exists {
			return nil, ErrDuplicateCapabilityInvoker
		}
		invokers[registration.Key] = registration.Invoker
	}
	return &CapabilityRegistryRouter{invokers: invokers}, nil
}

// Dispatch resolves and executes an immediate binding from snapshot.
func (router *CapabilityRegistryRouter) Dispatch(ctx context.Context, snapshot BindingCatalogSnapshot, name string, arguments any) (CapabilityDispatch, CapabilityDispatchFailure) {
	return router.dispatch(ctx, snapshot, name, arguments, nil)
}

// DispatchReviewed resolves and executes a reviewed binding with the exact
// durable authorization and argument digest supplied by the runtime.
func (router *CapabilityRegistryRouter) DispatchReviewed(ctx context.Context, snapshot BindingCatalogSnapshot, name string, arguments any, authorization ReviewedAuthorization) (CapabilityDispatch, CapabilityDispatchFailure) {
	return router.dispatch(ctx, snapshot, name, arguments, &authorization)
}

func (router *CapabilityRegistryRouter) dispatch(ctx context.Context, snapshot BindingCatalogSnapshot, name string, arguments any, authorization *ReviewedAuthorization) (CapabilityDispatch, CapabilityDispatchFailure) {
	binding, ok := snapshot.Resolve(name)
	if !ok {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityUnknownOperation, Binding{}, arguments)
	}
	if router == nil {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityUnknownInvoker, binding, arguments)
	}
	if binding.ReviewRoute == "" && authorization != nil {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityDenied, binding, arguments)
	}
	if binding.ReviewRoute != "" && authorization == nil {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityDenied, binding, arguments)
	}
	if binding.ReviewRoute != "" && binding.Destination == nil {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityDenied, binding, arguments)
	}
	if authorization != nil && !authorization.MatchesArguments(arguments) {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityDenied, binding, arguments)
	}
	argumentBytes, err := json.Marshal(arguments)
	if err != nil {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityInvalidArguments, binding, arguments)
	}
	var argumentValue any
	if err := json.Unmarshal(argumentBytes, &argumentValue); err != nil || !binding.InputCheck.Accepts(argumentValue) {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityInvalidArguments, binding, arguments)
	}
	if err := ValidateArguments(binding.InputSchema, argumentBytes); err != nil {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityInvalidArguments, binding, arguments)
	}
	invoker, ok := router.invokers[InvokerKey(binding.InvokerKey)]
	if !ok || invoker == nil {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityUnknownInvoker, binding, arguments)
	}
	invocation := CapabilityInvocation{Operation: binding.Name, OperationToken: binding.OperationToken, Arguments: arguments, ReviewedAuthorization: authorization}
	output, err := invoker.Invoke(ctx, invocation)
	if err != nil {
		safeError := capabilityError(err)
		return CapabilityDispatch{}, capabilityFailure(safeError, binding, arguments)
	}
	output = output.materializeFailure()
	persistedOutput := output.Payload
	if output.PersistedOutputSet {
		persistedOutput = output.PersistedOutput
	}
	return CapabilityDispatch{Output: output, Persisted: binding.PersistedViews(arguments, persistedOutput)}, CapabilityDispatchFailure{}
}

func cloneCapabilityValue(value any) any {
	raw, err := json.Marshal(value)
	if err != nil {
		return value
	}
	var clone any
	if err := json.Unmarshal(raw, &clone); err != nil {
		return value
	}
	return clone
}

func capabilityFailure(err error, binding Binding, arguments any) CapabilityDispatchFailure {
	if binding.Name == "" {
		return CapabilityDispatchFailure{Error: err, Persisted: PersistedBindingViews{}}
	}
	return CapabilityDispatchFailure{Error: err, Persisted: binding.PersistedViews(arguments, capabilityErrorPayload(err))}
}

func capabilityError(err error) error {
	switch {
	case errors.Is(err, ErrCapabilityUnknownInvoker):
		return ErrCapabilityUnknownInvoker
	case errors.Is(err, ErrCapabilityUnknownOperation):
		return ErrCapabilityUnknownOperation
	case errors.Is(err, ErrCapabilityInvalidArguments):
		return ErrCapabilityInvalidArguments
	case errors.Is(err, ErrCapabilityDenied):
		return ErrCapabilityDenied
	case errors.Is(err, ErrCapabilityUnavailable):
		return ErrCapabilityUnavailable
	case errors.Is(err, ErrCapabilityAuthenticationRequired):
		return ErrCapabilityAuthenticationRequired
	case errors.Is(err, ErrCapabilityOutcomeUncertain):
		return ErrCapabilityOutcomeUncertain
	default:
		return ErrCapabilityFailed
	}
}

func capabilityErrorPayload(err error) map[string]any {
	code, recovery := "failed", "retry_later"
	switch {
	case errors.Is(err, ErrCapabilityUnknownInvoker):
		code = "unknown_invoker"
	case errors.Is(err, ErrCapabilityUnknownOperation):
		code, recovery = "unknown_operation", "stop"
	case errors.Is(err, ErrCapabilityInvalidArguments):
		code, recovery = "invalid_arguments", "correct_arguments"
	case errors.Is(err, ErrCapabilityDenied):
		code, recovery = "denied", "stop"
	case errors.Is(err, ErrCapabilityUnavailable):
		code = "unavailable"
	case errors.Is(err, ErrCapabilityAuthenticationRequired):
		code, recovery = "authentication_required", "stop"
	case errors.Is(err, ErrCapabilityOutcomeUncertain):
		code, recovery = "outcome_uncertain", "stop"
	}
	return map[string]any{"error": code, "message": err.Error(), "recovery": recovery}
}

// MatchesArguments checks the exact canonical JSON argument digest.
func (authorization ReviewedAuthorization) MatchesArguments(arguments any) bool {
	encoded, err := json.Marshal(arguments)
	if err != nil {
		return false
	}
	digest := sha256.Sum256(encoded)
	return authorization.ArgumentsSHA256 == hex.EncodeToString(digest[:])
}
