package store

import (
	"errors"
	"fmt"
	"strings"
)

const maxCapabilityAuthorityComponentBytes = 512

// CapabilityAuthenticationAuthorityKind identifies the connection authority
// that must be authenticated before one exact capability can resume.
type CapabilityAuthenticationAuthorityKind string

const (
	CapabilityAuthenticationMCPServer         CapabilityAuthenticationAuthorityKind = "mcp_server"
	CapabilityAuthenticationAdapterConnection CapabilityAuthenticationAuthorityKind = "adapter_connection"
	CapabilityAuthenticationAdapterGrant      CapabilityAuthenticationAuthorityKind = "adapter_grant"
)

// CapabilityAuthenticationChallengeKind identifies the required interaction.
type CapabilityAuthenticationChallengeKind string

const (
	CapabilityAuthenticationReauthenticate    CapabilityAuthenticationChallengeKind = "reauthenticate"
	CapabilityAuthenticationReplaceCredential CapabilityAuthenticationChallengeKind = "replace_credential"
)

// CapabilityAuthenticationChallenge is the bounded, secret-free identity of
// one authentication interruption.
type CapabilityAuthenticationChallenge struct {
	ChallengeKind     CapabilityAuthenticationChallengeKind `json:"challenge_kind"`
	AuthorityKind     CapabilityAuthenticationAuthorityKind `json:"authority_kind"`
	AuthorityID       string                                `json:"authority_id"`
	DestinationID     string                                `json:"destination_id,omitempty"`
	AuthorityRevision string                                `json:"authority_revision"`
}

// CapabilityAuthenticationChallengeError reports an invalid identity value.
type CapabilityAuthenticationChallengeError struct{}

func (CapabilityAuthenticationChallengeError) Error() string {
	return "capability authentication challenge identity is invalid"
}

// NewCapabilityAuthenticationChallenge constructs a bounded authentication
// challenge containing only stable, non-secret identity.
func NewCapabilityAuthenticationChallenge(kind CapabilityAuthenticationChallengeKind, authorityKind CapabilityAuthenticationAuthorityKind, authorityID, revision string) (CapabilityAuthenticationChallenge, error) {
	if err := validateCapabilityAuthorityComponent(authorityID, maxCapabilityAuthorityComponentBytes); err != nil {
		return CapabilityAuthenticationChallenge{}, CapabilityAuthenticationChallengeError{}
	}
	if err := validateCapabilityAuthorityComponent(revision, maxCapabilityAuthorityComponentBytes); err != nil {
		return CapabilityAuthenticationChallenge{}, CapabilityAuthenticationChallengeError{}
	}
	return CapabilityAuthenticationChallenge{ChallengeKind: kind, AuthorityKind: authorityKind, AuthorityID: authorityID, AuthorityRevision: revision}, nil
}

// NewCapabilityAuthenticationChallengeForDestination constructs a challenge
// whose reusable authority differs from its concrete destination.
func NewCapabilityAuthenticationChallengeForDestination(kind CapabilityAuthenticationChallengeKind, authorityKind CapabilityAuthenticationAuthorityKind, authorityID, destinationID, revision string) (CapabilityAuthenticationChallenge, error) {
	challenge, err := NewCapabilityAuthenticationChallenge(kind, authorityKind, authorityID, revision)
	if err != nil {
		return CapabilityAuthenticationChallenge{}, err
	}
	if err := validateCapabilityAuthorityComponent(destinationID, maxCapabilityAuthorityComponentBytes); err != nil {
		return CapabilityAuthenticationChallenge{}, CapabilityAuthenticationChallengeError{}
	}
	challenge.DestinationID = destinationID
	return challenge, nil
}

// Destination returns the concrete destination or the authority itself.
func (c CapabilityAuthenticationChallenge) Destination() string {
	if c.DestinationID != "" {
		return c.DestinationID
	}
	return c.AuthorityID
}

const maxCapabilityDestinationComponentBytes = 256

// CapabilityDestination is one exact non-secret destination captured by a
// capability binding.
type CapabilityDestination struct {
	ServiceID              string  `json:"service_id"`
	ConnectionID           string  `json:"connection_id"`
	AccountID              *string `json:"account_id,omitempty"`
	Revision               string  `json:"revision"`
	AuthenticationRevision *string `json:"authentication_revision,omitempty"`
}

// CapabilityDestinationError reports the component that failed validation.
type CapabilityDestinationError struct {
	Kind  string
	Field string
}

func (e CapabilityDestinationError) Error() string {
	switch e.Kind {
	case "blank":
		return fmt.Sprintf("capability destination component is blank: %s", e.Field)
	case "too_long":
		return fmt.Sprintf("capability destination component is too long: %s", e.Field)
	default:
		return fmt.Sprintf("capability destination component is invalid: %s", e.Field)
	}
}

// NewCapabilityDestination constructs one exact destination identity.
func NewCapabilityDestination(serviceID, connectionID string, accountID *string, revision string) (CapabilityDestination, error) {
	serviceID, err := validateCapabilityDestinationComponent("service_id", serviceID)
	if err != nil {
		return CapabilityDestination{}, err
	}
	connectionID, err = validateCapabilityDestinationComponent("connection_id", connectionID)
	if err != nil {
		return CapabilityDestination{}, err
	}
	if accountID != nil {
		value, accountErr := validateCapabilityDestinationComponent("account_id", *accountID)
		if accountErr != nil {
			return CapabilityDestination{}, accountErr
		}
		accountID = &value
	}
	revision, err = validateCapabilityDestinationComponent("revision", revision)
	if err != nil {
		return CapabilityDestination{}, err
	}
	return CapabilityDestination{ServiceID: serviceID, ConnectionID: connectionID, AccountID: accountID, Revision: revision}, nil
}

// WithAuthenticationRevision adds the stable credential successor fence.
func (d CapabilityDestination) WithAuthenticationRevision(revision string) (CapabilityDestination, error) {
	value, err := validateCapabilityDestinationComponent("authentication_revision", revision)
	if err != nil {
		return CapabilityDestination{}, err
	}
	d.AuthenticationRevision = &value
	return d, nil
}

// AuthenticationFence returns the explicit successor fence or the destination revision.
func (d CapabilityDestination) AuthenticationFence() string {
	if d.AuthenticationRevision != nil {
		return *d.AuthenticationRevision
	}
	return d.Revision
}

func validateCapabilityAuthorityComponent(value string, limit int) error {
	if value == "" || len([]byte(value)) > limit {
		return errors.New("invalid capability authority component")
	}
	for _, character := range value {
		if !((character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') || (character >= '0' && character <= '9') || strings.ContainsRune(":_-./", character)) {
			return errors.New("invalid capability authority component")
		}
	}
	return nil
}

func validateCapabilityDestinationComponent(field, value string) (string, error) {
	if value == "" || strings.TrimSpace(value) != value {
		return "", CapabilityDestinationError{Kind: "blank", Field: field}
	}
	if len([]byte(value)) > maxCapabilityDestinationComponentBytes {
		return "", CapabilityDestinationError{Kind: "too_long", Field: field}
	}
	for _, character := range value {
		if !((character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') || (character >= '0' && character <= '9') || strings.ContainsRune("._:-/", character)) {
			return "", CapabilityDestinationError{Kind: "invalid", Field: field}
		}
	}
	return value, nil
}
