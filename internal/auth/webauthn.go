package auth

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"sync"
	"time"

	"github.com/go-webauthn/webauthn/protocol"
	webauthnlib "github.com/go-webauthn/webauthn/webauthn"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	ceremonyLifetime = 5 * time.Minute
	ceremonyCapacity = 64
)

var (
	errCeremonyFull    = errors.New("passkey ceremony capacity reached")
	errInvalidCeremony = errors.New("invalid passkey ceremony")
	errPasskeyState    = errors.New("stored passkey state is invalid")
)

type ceremonyKind int

const (
	registrationCeremony ceremonyKind = iota + 1
	authenticationCeremony
)

type pendingCeremony struct {
	kind        ceremonyKind
	binding     [32]byte
	expiresAt   time.Time
	session     webauthnlib.SessionData
	credentials []store.HumanPasskey
	authority   store.RegistrationAuthority
}

type passkeySecurity struct {
	store      *store.Store
	webauthn   *webauthnlib.WebAuthn
	mu         sync.Mutex
	ceremonies map[string]pendingCeremony
}

type localHuman struct {
	credentials []webauthnlib.Credential
}

func newPasskeySecurity(config Config, taskStore *store.Store) (*passkeySecurity, error) {
	verifier, err := webauthnlib.New(&webauthnlib.Config{
		RPID:          config.RPID,
		RPDisplayName: "Noema",
		RPOrigins:     []string{config.Origin},
		AuthenticatorSelection: protocol.AuthenticatorSelection{
			UserVerification: protocol.VerificationRequired,
		},
		Timeouts: webauthnlib.TimeoutsConfig{
			Login: webauthnlib.TimeoutConfig{
				Enforce: true,
				Timeout: ceremonyLifetime,
			},
			Registration: webauthnlib.TimeoutConfig{
				Enforce: true,
				Timeout: ceremonyLifetime,
			},
		},
	})
	if err != nil {
		return nil, err
	}
	return &passkeySecurity{
		store:      taskStore,
		webauthn:   verifier,
		ceremonies: make(map[string]pendingCeremony),
	}, nil
}

func (p *passkeySecurity) beginRegistration(
	r *http.Request,
	browser browserSession,
	devNoAuth bool,
	setupAuthorized bool,
) (any, string, error) {
	stored, user, err := p.loadUser(r)
	if err != nil {
		return nil, "", err
	}
	authority := store.RegistrationInitial
	if len(stored) > 0 {
		switch {
		case devNoAuth:
			authority = store.RegistrationBypass
		case browser.record.State == "authenticated" &&
			!browser.record.RecentPasskeyAt.After(time.Now()) &&
			time.Since(browser.record.RecentPasskeyAt) <= 5*time.Minute:
			authority = store.RegistrationCurrent
		case setupAuthorized:
			authority = store.RegistrationSetup
		default:
			return nil, "", store.ErrSessionUnauthorized
		}
	}
	exclusions := make([]protocol.CredentialDescriptor, 0, len(user.credentials))
	for _, credential := range user.credentials {
		exclusions = append(exclusions, credential.Descriptor())
	}
	options, session, err := p.webauthn.BeginRegistration(
		user,
		webauthnlib.WithExclusions(exclusions),
		webauthnlib.WithAuthenticatorSelection(protocol.AuthenticatorSelection{
			UserVerification: protocol.VerificationRequired,
		}),
	)
	if err != nil {
		return nil, "", err
	}
	id, err := p.insert(pendingCeremony{
		kind:      registrationCeremony,
		binding:   browser.digest,
		expiresAt: time.Now().Add(ceremonyLifetime),
		session:   *session,
		authority: authority,
	})
	if err != nil {
		return nil, "", err
	}
	return options, id, nil
}

func (p *passkeySecurity) finishRegistration(
	r *http.Request,
	browser browserSession,
	ceremonyID string,
	credential json.RawMessage,
) (store.HumanPasskey, store.RegistrationAuthority, error) {
	pending, err := p.take(ceremonyID, browser.digest, registrationCeremony)
	if err != nil {
		return store.HumanPasskey{}, "", err
	}
	_, user, err := p.loadUser(r)
	if err != nil {
		return store.HumanPasskey{}, "", err
	}
	request := credentialRequest(r, credential)
	created, err := p.webauthn.FinishRegistration(user, pending.session, request)
	if err != nil {
		return store.HumanPasskey{}, "", err
	}
	encoded, err := json.Marshal(created)
	if err != nil {
		return store.HumanPasskey{}, "", err
	}
	return store.HumanPasskey{
		CredentialID:   base64.RawURLEncoding.EncodeToString(created.ID),
		CredentialJSON: string(encoded),
	}, pending.authority, nil
}

func (p *passkeySecurity) beginAuthentication(
	r *http.Request,
	browser browserSession,
) (any, string, error) {
	stored, user, err := p.loadUser(r)
	if err != nil {
		return nil, "", err
	}
	if len(stored) == 0 {
		return nil, "", store.ErrInitialPasskeyClaimed
	}
	options, session, err := p.webauthn.BeginLogin(
		user,
		webauthnlib.WithUserVerification(protocol.VerificationRequired),
	)
	if err != nil {
		return nil, "", err
	}
	id, err := p.insert(pendingCeremony{
		kind:        authenticationCeremony,
		binding:     browser.digest,
		expiresAt:   time.Now().Add(ceremonyLifetime),
		session:     *session,
		credentials: stored,
	})
	if err != nil {
		return nil, "", err
	}
	return options, id, nil
}

func (p *passkeySecurity) finishAuthentication(
	r *http.Request,
	browser browserSession,
	ceremonyID string,
	credential json.RawMessage,
) (store.HumanPasskey, string, error) {
	pending, err := p.take(ceremonyID, browser.digest, authenticationCeremony)
	if err != nil {
		return store.HumanPasskey{}, "", err
	}
	user, err := userFromStored(pending.credentials)
	if err != nil {
		return store.HumanPasskey{}, "", err
	}
	validated, err := p.webauthn.FinishLogin(user, pending.session, credentialRequest(r, credential))
	if err != nil {
		return store.HumanPasskey{}, "", err
	}
	id := base64.RawURLEncoding.EncodeToString(validated.ID)
	var expected string
	for _, stored := range pending.credentials {
		if stored.CredentialID == id {
			expected = stored.CredentialJSON
			break
		}
	}
	if expected == "" {
		return store.HumanPasskey{}, "", store.ErrCredentialChanged
	}
	encoded, err := json.Marshal(validated)
	if err != nil {
		return store.HumanPasskey{}, "", err
	}
	return store.HumanPasskey{CredentialID: id, CredentialJSON: string(encoded)}, expected, nil
}

func (p *passkeySecurity) insert(ceremony pendingCeremony) (string, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	now := time.Now()
	for id, pending := range p.ceremonies {
		if !pending.expiresAt.After(now) || pending.binding == ceremony.binding {
			delete(p.ceremonies, id)
		}
	}
	if len(p.ceremonies) >= ceremonyCapacity {
		return "", errCeremonyFull
	}
	id, err := randomBase64URL(32)
	if err != nil {
		return "", err
	}
	p.ceremonies[id] = ceremony
	return id, nil
}

func (p *passkeySecurity) take(
	id string,
	binding [32]byte,
	kind ceremonyKind,
) (pendingCeremony, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	pending, exists := p.ceremonies[id]
	if !exists || pending.kind != kind || !bytes.Equal(pending.binding[:], binding[:]) {
		return pendingCeremony{}, errInvalidCeremony
	}
	delete(p.ceremonies, id)
	if !pending.expiresAt.After(time.Now()) {
		return pendingCeremony{}, errInvalidCeremony
	}
	return pending, nil
}

func (p *passkeySecurity) clear() {
	p.mu.Lock()
	clear(p.ceremonies)
	p.mu.Unlock()
}

func (p *passkeySecurity) loadUser(r *http.Request) ([]store.HumanPasskey, *localHuman, error) {
	stored, err := p.store.Passkeys(r.Context())
	if err != nil {
		return nil, nil, err
	}
	user, err := userFromStored(stored)
	return stored, user, err
}

func userFromStored(stored []store.HumanPasskey) (*localHuman, error) {
	credentials := make([]webauthnlib.Credential, 0, len(stored))
	for _, item := range stored {
		var credential webauthnlib.Credential
		if err := json.Unmarshal([]byte(item.CredentialJSON), &credential); err != nil {
			return nil, errPasskeyState
		}
		credentials = append(credentials, credential)
	}
	return &localHuman{credentials: credentials}, nil
}

func credentialRequest(source *http.Request, credential json.RawMessage) *http.Request {
	request := source.Clone(source.Context())
	request.Body = http.NoBody
	if len(credential) > 0 {
		request.Body = io.NopCloser(bytes.NewReader(credential))
	}
	request.ContentLength = int64(len(credential))
	request.Header = source.Header.Clone()
	request.Header.Set("Content-Type", "application/json")
	return request
}

func (u *localHuman) WebAuthnID() []byte {
	return []byte{0x6e, 0x6f, 0x65, 0x6d, 0x61, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01}
}

func (*localHuman) WebAuthnName() string { return "human:local" }

func (*localHuman) WebAuthnDisplayName() string { return "You" }

func (u *localHuman) WebAuthnCredentials() []webauthnlib.Credential { return u.credentials }
