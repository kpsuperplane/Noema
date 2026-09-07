package provider

import (
	"context"
	"errors"
	"sync"
)

// ProviderRuntime owns the process-wide provider admission and routing state.
// Account and generation services share this instance so a test or a live
// request cannot accidentally create a second authority for the same account.
type ProviderRuntime struct {
	accountMu    sync.Mutex
	accountGates map[string]*sync.Mutex
	generation   *ProviderGenerationArbiter
	registry     *providerRegistry
	closed       bool
}

func newProviderRuntime() *ProviderRuntime {
	return &ProviderRuntime{
		accountGates: make(map[string]*sync.Mutex),
		generation:   newProviderGenerationArbiter(),
		registry:     newProviderRegistry(),
	}
}

func (runtime *ProviderRuntime) accountGate(id string) *sync.Mutex {
	runtime.accountMu.Lock()
	defer runtime.accountMu.Unlock()
	if runtime.accountGates[id] == nil {
		runtime.accountGates[id] = &sync.Mutex{}
	}
	return runtime.accountGates[id]
}

func (runtime *ProviderRuntime) admitGeneration(ctx context.Context, priority int) (func(), error) {
	if runtime == nil || runtime.generation == nil {
		return func() {}, nil
	}
	return runtime.generation.acquire(ctx, priority)
}

// RegisterGenerator records one live provider instance in the runtime registry.
// The registry tracks replacement generations and leases used by routing.
func (runtime *ProviderRuntime) RegisterGenerator(key string, generator Generator) (func() error, error) {
	if runtime == nil || runtime.registry == nil || generator == nil {
		return nil, errors.New("provider generator registration is invalid")
	}
	key, err := ProviderAccountInstanceKey(key)
	if err != nil {
		return nil, errors.New("provider generator registration is invalid")
	}
	drops := new(int)
	registration, err := runtime.registry.registration(key, providerTrackedProvider{label: key, generator: generator, drops: drops, mu: &sync.Mutex{}})
	if err != nil {
		return nil, err
	}
	return func() error { return providerRetireRegistration(runtime.registry, registration) }, nil
}

// LeaseGenerator returns the exact registered generation for one instance.
// The release function must run when the request no longer uses the generator.
func (runtime *ProviderRuntime) LeaseGenerator(key string) (Generator, func(), error) {
	if runtime == nil || runtime.registry == nil {
		return nil, nil, errors.New("provider generator selection is invalid")
	}
	key, err := ProviderAccountInstanceKey(key)
	if err != nil {
		return nil, nil, errors.New("provider generator selection is invalid")
	}
	lease, err := runtime.registry.lease(key)
	if err != nil {
		return nil, nil, err
	}
	if lease.entry == nil || lease.entry.provider.generator == nil {
		lease.release()
		return nil, nil, errors.New("provider generator is unavailable")
	}
	return lease.entry.provider.generator, lease.release, nil
}

// Close drains account-independent generation waiters and prevents new ones.
func (runtime *ProviderRuntime) Close() {
	if runtime == nil {
		return
	}
	runtime.accountMu.Lock()
	if runtime.closed {
		runtime.accountMu.Unlock()
		return
	}
	runtime.closed = true
	runtime.accountMu.Unlock()
	if runtime.generation != nil {
		runtime.generation.close()
	}
}
