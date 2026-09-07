package main

type runtimeHostErrorKind uint8

const (
	runtimeHostStoreError runtimeHostErrorKind = iota + 1
	runtimeHostCompositionError
)

// RuntimeHostError classifies failures while assembling the local host.
type RuntimeHostError struct {
	kind   runtimeHostErrorKind
	cause  error
	detail string
}

// NewRuntimeHostStoreError wraps a local store startup failure.
func NewRuntimeHostStoreError(cause error) RuntimeHostError {
	return RuntimeHostError{kind: runtimeHostStoreError, cause: cause}
}

// NewRuntimeHostCompositionError wraps a host composition failure.
func NewRuntimeHostCompositionError(detail string) RuntimeHostError {
	return RuntimeHostError{kind: runtimeHostCompositionError, detail: detail}
}

func (e RuntimeHostError) Error() string {
	switch e.kind {
	case runtimeHostStoreError:
		if e.cause != nil {
			return "store: " + e.cause.Error()
		}
		return "store startup failed"
	case runtimeHostCompositionError:
		return "composition: " + e.detail
	default:
		return "host startup failed"
	}
}

func (e RuntimeHostError) Unwrap() error { return e.cause }

// UserMessage returns the stable plain-language startup message.
func (e RuntimeHostError) UserMessage() string {
	switch e.kind {
	case runtimeHostStoreError:
		return "Noema could not start its local memory store."
	case runtimeHostCompositionError:
		return "Noema could not start the local assistant service."
	default:
		return "Noema could not start the local assistant service."
	}
}

var _ error = RuntimeHostError{}
