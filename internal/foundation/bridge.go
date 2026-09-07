// Package foundation owns the host boundary for the Apple Foundation bridge.
package foundation

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os/exec"
)

const protocolVersion = 3

// Health contains the safe capabilities reported by the Foundation bridge.
type Health struct {
	Available bool
	Profiles  []Profile
}

// Profile identifies one Foundation model profile.
type Profile struct {
	ID    string `json:"id"`
	Label string `json:"label"`
}

// DefaultSelection is the durable provider choice produced by Foundation
// readiness resolution.
type DefaultSelection struct {
	ProviderKind string
	ModelProfile string
}

// ResolveDefaultSelection probes the bridge and selects one configured
// Foundation profile.
func ResolveDefaultSelection(ctx context.Context, path, configuredProfile string) (DefaultSelection, error) {
	health, err := Probe(ctx, path)
	if err != nil {
		return DefaultSelection{}, err
	}
	for _, profile := range health.Profiles {
		if profile.ID == configuredProfile {
			return DefaultSelection{ProviderKind: "foundation_local", ModelProfile: profile.ID}, nil
		}
	}
	return DefaultSelection{}, &BridgeError{Kind: "profile unavailable", Detail: configuredProfile}
}

// BridgeError identifies one bridge lifecycle or protocol failure.
type BridgeError struct {
	Kind   string
	Detail string
}

func (e *BridgeError) Error() string {
	if e.Detail == "" {
		return "foundation bridge " + e.Kind
	}
	return "foundation bridge " + e.Kind + ": " + e.Detail
}

// Probe starts one configured bridge and verifies its protocol and health.
func Probe(ctx context.Context, path string) (Health, error) {
	if path == "" {
		return Health{}, &BridgeError{Kind: "binary is missing"}
	}
	command := exec.CommandContext(ctx, path)
	stdin, err := command.StdinPipe()
	if err != nil {
		return Health{}, &BridgeError{Kind: "launch failed", Detail: err.Error()}
	}
	stdout, err := command.StdoutPipe()
	if err != nil {
		return Health{}, &BridgeError{Kind: "launch failed", Detail: err.Error()}
	}
	command.Stderr = io.Discard
	if err := command.Start(); err != nil {
		return Health{}, &BridgeError{Kind: "launch failed", Detail: err.Error()}
	}
	defer func() {
		_ = stdin.Close()
		_ = command.Process.Kill()
		_ = command.Wait()
	}()
	encoder := json.NewEncoder(stdin)
	if err := encoder.Encode(map[string]any{"id": "handshake", "payload": map[string]any{"type": "handshake", "protocol_version": protocolVersion}}); err != nil {
		return Health{}, &BridgeError{Kind: "protocol failed", Detail: err.Error()}
	}
	scanner := bufio.NewScanner(stdout)
	response, err := nextResponse(scanner, "handshake")
	if err != nil {
		return Health{}, err
	}
	if response.Payload.Type != "handshake_ok" || response.Payload.ProtocolVersion != protocolVersion {
		return Health{}, &BridgeError{Kind: "protocol failed", Detail: "unsupported handshake response"}
	}
	if err := encoder.Encode(map[string]any{"id": "health", "payload": map[string]any{"type": "health"}}); err != nil {
		return Health{}, &BridgeError{Kind: "protocol failed", Detail: err.Error()}
	}
	response, err = nextResponse(scanner, "health")
	if err != nil {
		return Health{}, err
	}
	if response.Payload.Type != "health" {
		return Health{}, &BridgeError{Kind: "protocol failed", Detail: "unexpected health response"}
	}
	health := Health{Available: response.Payload.Available, Profiles: response.Payload.Profiles}
	if !health.Available {
		detail := response.Payload.UnavailableReason
		if detail == "" {
			detail = "Foundation Models runtime is unavailable."
		}
		return health, &BridgeError{Kind: "models unavailable", Detail: detail}
	}
	return health, nil
}

type response struct {
	ID      string  `json:"id"`
	Payload payload `json:"payload"`
}

type payload struct {
	Type              string    `json:"type"`
	ProtocolVersion   uint32    `json:"protocol_version"`
	Available         bool      `json:"available"`
	Profiles          []Profile `json:"profiles"`
	UnavailableReason string    `json:"unavailable_reason"`
}

func nextResponse(scanner *bufio.Scanner, expected string) (response, error) {
	if !scanner.Scan() {
		if err := scanner.Err(); err != nil {
			return response{}, &BridgeError{Kind: "protocol failed", Detail: err.Error()}
		}
		return response{}, &BridgeError{Kind: "protocol failed", Detail: "bridge closed stdout"}
	}
	var value response
	if err := json.Unmarshal(scanner.Bytes(), &value); err != nil {
		return response{}, &BridgeError{Kind: "protocol failed", Detail: err.Error()}
	}
	if value.ID != expected {
		return response{}, &BridgeError{Kind: "protocol failed", Detail: fmt.Sprintf("response id %q does not match %q", value.ID, expected)}
	}
	return value, nil
}
