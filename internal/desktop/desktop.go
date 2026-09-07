// Package desktop contains the local authorities used by the desktop client
// boundary. The package keeps credentials out of the desktop selection file
// and applies the same callback and transport validation as the native client.
package desktop

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"net/url"
	"os"
	"strings"
	"sync"

	"github.com/kpsuperplane/noema/internal/home"
)

// RemoteMetadata identifies a trusted remote server and its native client.
type RemoteMetadata struct {
	Origin   string `json:"origin"`
	ClientID string `json:"clientId"`
}

// RemoteProfile contains the protected remote credential and its in-memory
// access token. AccessToken is never persisted by ProfileStore.
type RemoteProfile struct {
	Metadata     RemoteMetadata
	RefreshToken string
	AccessToken  *string
}

// CredentialStore is the operating-system credential store boundary.
type CredentialStore interface {
	Get() (string, error)
	Set(string) error
	Delete() error
}

// SelectionKind describes the desktop connection selected at startup.
type SelectionKind string

const (
	SelectionLocal    SelectionKind = "local"
	SelectionRemote   SelectionKind = "remote"
	SelectionRecovery SelectionKind = "recovery"
)

// Selection is the safe result of loading the desktop connection selection.
type Selection struct {
	Kind    SelectionKind
	Profile *RemoteProfile
	Message string
}

type storedSelection struct {
	Mode string `json:"mode"`
}

type storedCredential struct {
	Origin       string `json:"origin"`
	ClientID     string `json:"clientId"`
	RefreshToken string `json:"refreshToken"`
}

// ProfileStore stores the non-secret selection in a private file and the
// remote credential in the operating-system credential store.
type ProfileStore struct {
	configPath string
	credential CredentialStore
}

// NewProfileStore creates a profile store for one desktop settings path.
func NewProfileStore(configPath string, credential CredentialStore) *ProfileStore {
	return &ProfileStore{configPath: configPath, credential: credential}
}

// Load loads the selected local or remote desktop mode.
func (s *ProfileStore) Load() Selection {
	data, err := os.ReadFile(s.configPath)
	if errors.Is(err, os.ErrNotExist) {
		return Selection{Kind: SelectionLocal}
	}
	if err != nil {
		return Selection{Kind: SelectionRecovery, Message: "Noema could not read its desktop connection settings."}
	}
	var selection storedSelection
	if json.Unmarshal(data, &selection) != nil {
		return Selection{Kind: SelectionRecovery, Message: "Noema could not read its desktop connection settings."}
	}
	switch selection.Mode {
	case "local":
		return Selection{Kind: SelectionLocal}
	case "remote":
		if s.credential == nil {
			return Selection{Kind: SelectionRecovery, Message: "Noema could not read the remote client credential from secure storage."}
		}
		protected, getErr := s.credential.Get()
		if getErr != nil {
			return Selection{Kind: SelectionRecovery, Message: "Noema could not read the remote client credential from secure storage."}
		}
		var saved storedCredential
		if json.Unmarshal([]byte(protected), &saved) != nil {
			return Selection{Kind: SelectionRecovery, Message: "Noema could not read the protected remote profile."}
		}
		if err := validateCredential(saved); err != nil {
			return Selection{Kind: SelectionRecovery, Message: err.Error()}
		}
		return Selection{Kind: SelectionRemote, Profile: &RemoteProfile{
			Metadata:     RemoteMetadata{Origin: saved.Origin, ClientID: saved.ClientID},
			RefreshToken: saved.RefreshToken,
		}}
	default:
		return Selection{Kind: SelectionRecovery, Message: "Noema could not read its desktop connection settings."}
	}
}

// SaveRemote stores only the remote selection marker in the settings file.
func (s *ProfileStore) SaveRemote(profile RemoteProfile) error {
	if s.credential == nil {
		return errors.New("credential store unavailable")
	}
	saved := storedCredential{Origin: profile.Metadata.Origin, ClientID: profile.Metadata.ClientID, RefreshToken: profile.RefreshToken}
	if err := validateCredential(saved); err != nil {
		return err
	}
	protected, err := json.Marshal(saved)
	if err != nil {
		return errors.New("Noema could not encode the protected remote profile.")
	}
	if err := s.credential.Set(string(protected)); err != nil {
		return errors.New("Noema could not store the remote client credential securely.")
	}
	if err := s.writeSelection(storedSelection{Mode: "remote"}); err != nil {
		_ = s.credential.Delete()
		return err
	}
	return nil
}

func (s *ProfileStore) writeSelection(selection storedSelection) error {
	data, err := json.MarshalIndent(selection, "", "  ")
	if err != nil {
		return errors.New("Noema could not encode its desktop connection settings.")
	}
	data = append(data, '\n')
	if err := home.AtomicWritePrivate(s.configPath, data); err != nil {
		return errors.New("Noema could not save its desktop connection settings.")
	}
	return nil
}

func validateCredential(value storedCredential) error {
	if value.Origin == "" || len(value.Origin) > 2048 || value.ClientID == "" || len(value.ClientID) > 128 || value.RefreshToken == "" || len(value.RefreshToken) > 128 {
		return errors.New("The protected remote profile is invalid.")
	}
	return nil
}

// SubscriptionTasks tracks one generation for each subscription identifier.
// A finished task can remove only the generation that created it.
type SubscriptionTasks struct {
	mu             sync.Mutex
	nextGeneration uint64
	entries        map[string]subscriptionTask
}

type subscriptionTask struct {
	generation uint64
	cancel     context.CancelFunc
}

// NewSubscriptionTasks returns an empty subscription registry.
func NewSubscriptionTasks() *SubscriptionTasks {
	return &SubscriptionTasks{entries: make(map[string]subscriptionTask)}
}

// Insert replaces an identifier and returns its new generation.
func (s *SubscriptionTasks) Insert(id string, cancel context.CancelFunc) uint64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.entries == nil {
		s.entries = make(map[string]subscriptionTask)
	}
	if previous, ok := s.entries[id]; ok && previous.cancel != nil {
		previous.cancel()
	}
	if s.nextGeneration == ^uint64(0) {
		panic("subscription generation counter exhausted")
	}
	s.nextGeneration++
	s.entries[id] = subscriptionTask{generation: s.nextGeneration, cancel: cancel}
	return s.nextGeneration
}

// RemoveFinished removes an entry only when its generation is still current.
func (s *SubscriptionTasks) RemoveFinished(id string, generation uint64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if entry, ok := s.entries[id]; ok && entry.generation == generation {
		delete(s.entries, id)
	}
}

// Generation returns the current generation for one subscription.
func (s *SubscriptionTasks) Generation(id string) (uint64, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	entry, ok := s.entries[id]
	if !ok {
		return 0, false
	}
	return entry.generation, true
}

// Len returns the number of active subscription entries.
func (s *SubscriptionTasks) Len() int {
	s.mu.Lock()
	defer s.mu.Unlock()
	return len(s.entries)
}

// RemoteError classifies a remote operation failure.
type RemoteError string

const (
	RemoteUnauthorized    RemoteError = "unauthorized"
	RemoteOffline         RemoteError = "offline"
	RemoteInvalidResponse RemoteError = "invalid_response"
)

func (e RemoteError) Error() string { return string(e) }

// AllowLocalAfterRemote allows local mode after a successful revocation or
// when the server already rejected the remote credential.
func AllowLocalAfterRemote(err error) error {
	if err == nil || errors.Is(err, RemoteUnauthorized) {
		return nil
	}
	return errors.New("Noema could not revoke this client. Forget the server only if you cannot reconnect.")
}

// SubscriptionEventPayload is the frontend wire payload.
type SubscriptionEventPayload struct {
	SubscriptionID string          `json:"subscriptionId"`
	Response       json.RawMessage `json:"response"`
}

// ConnectionChangedPayload is the frontend connection state payload.
type ConnectionChangedPayload struct {
	State string `json:"state"`
}

// RequireMainWindowLabel limits desktop IPC commands to the main window.
func RequireMainWindowLabel(label string) error {
	if label == "main" {
		return nil
	}
	return errors.New("Noema rejected a request from an unauthorized app window.")
}

// IsExternalWebURL accepts only HTTP and HTTPS URLs with an authority.
func IsExternalWebURL(raw string) bool {
	parsed, err := url.Parse(raw)
	return err == nil && parsed.Host != "" && (parsed.Scheme == "http" || parsed.Scheme == "https")
}

// RemoteGraphQL builds authenticated requests for one trusted origin.
type RemoteGraphQL struct {
	origin   string
	profile  RemoteProfile
	profiles *ProfileStore
}

// NewRemoteGraphQL prepares one profile-backed remote HTTPS connection.
func NewRemoteGraphQL(profile RemoteProfile, profiles *ProfileStore) (*RemoteGraphQL, error) {
	origin, err := TrustedOrigin(profile.Metadata.Origin)
	if err != nil {
		return nil, err
	}
	return &RemoteGraphQL{origin: origin, profile: profile, profiles: profiles}, nil
}

// NewLocalGraphQL validates one loopback HTTP origin.
func NewLocalGraphQL(raw, _ string) (*RemoteGraphQL, error) {
	origin, err := localOrigin(raw)
	if err != nil {
		return nil, err
	}
	return &RemoteGraphQL{origin: origin}, nil
}

// BuildGraphQLRequest preserves the JSON body and attaches a bearer token.
func (r *RemoteGraphQL) BuildGraphQLRequest(body []byte, access string) (*http.Request, error) {
	request, err := http.NewRequest(http.MethodPost, r.origin+"/graphql", bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	request.Header.Set("Authorization", "Bearer "+access)
	request.Header.Set("Content-Type", "application/json")
	return request, nil
}

// Origin returns the normalized HTTP origin.
func (r *RemoteGraphQL) Origin() string { return r.origin }

// WebsocketURL returns the GraphQL transport WebSocket endpoint.
func (r *RemoteGraphQL) WebsocketURL() string {
	if strings.HasPrefix(r.origin, "http://") {
		return "ws://" + strings.TrimPrefix(r.origin, "http://") + "/graphql/ws"
	}
	return "wss://" + strings.TrimPrefix(r.origin, "https://") + "/graphql/ws"
}

// ProtocolMessageKind identifies one GraphQL WebSocket message.
type ProtocolMessageKind string

const (
	ProtocolNext     ProtocolMessageKind = "next"
	ProtocolComplete ProtocolMessageKind = "complete"
	ProtocolPing     ProtocolMessageKind = "ping"
	ProtocolIgnore   ProtocolMessageKind = "ignore"
)

// ProtocolMessage is the normalized GraphQL WebSocket message.
type ProtocolMessage struct {
	Kind  ProtocolMessageKind
	Value map[string]any
}

// MapProtocolMessage translates the server protocol into frontend events.
func MapProtocolMessage(raw string) ProtocolMessage {
	var value map[string]any
	if json.Unmarshal([]byte(raw), &value) != nil {
		return ProtocolMessage{Kind: ProtocolIgnore}
	}
	kind, _ := value["type"].(string)
	switch kind {
	case "next":
		payload, ok := value["payload"].(map[string]any)
		if !ok {
			return ProtocolMessage{Kind: ProtocolIgnore}
		}
		return ProtocolMessage{Kind: ProtocolNext, Value: payload}
	case "error":
		return ProtocolMessage{Kind: ProtocolNext, Value: map[string]any{"errors": value["payload"]}}
	case "complete":
		return ProtocolMessage{Kind: ProtocolComplete}
	case "ping":
		return ProtocolMessage{Kind: ProtocolPing}
	default:
		return ProtocolMessage{Kind: ProtocolIgnore}
	}
}

func localOrigin(raw string) (string, error) {
	parsed, err := url.Parse(raw)
	if err != nil || parsed.Scheme != "http" || parsed.Hostname() != "127.0.0.1" || parsed.Port() == "" {
		return "", errors.New("Noema received an invalid local server address.")
	}
	origin := "http://" + parsed.Host
	if strings.TrimRight(raw, "/") != origin {
		return "", errors.New("Noema received an invalid local server address.")
	}
	return origin, nil
}

// RequestHead is the bounded loopback HTTP request line and headers.
type RequestHead struct {
	Method string
	Target string
	Host   string
}

// ParseRequestHead parses one HTTP/1.1 request head without reading a body.
func ParseRequestHead(raw string) (RequestHead, error) {
	end := strings.Index(raw, "\r\n\r\n")
	if end < 0 {
		return RequestHead{}, errors.New("invalid request")
	}
	lines := strings.Split(raw[:end], "\r\n")
	fields := strings.Fields(lines[0])
	if len(fields) != 3 || fields[2] != "HTTP/1.1" {
		return RequestHead{}, errors.New("invalid request")
	}
	request := RequestHead{Method: fields[0], Target: fields[1]}
	for _, line := range lines[1:] {
		name, value, ok := strings.Cut(line, ":")
		if !ok {
			return RequestHead{}, errors.New("invalid request")
		}
		if strings.EqualFold(strings.TrimSpace(name), "Host") {
			request.Host = strings.TrimSpace(value)
		}
	}
	if request.Method == "" || request.Target == "" {
		return RequestHead{}, errors.New("invalid request")
	}
	return request, nil
}

// Path returns the decoded request path.
func (r RequestHead) Path() string {
	parsed, err := url.ParseRequestURI(r.Target)
	if err != nil {
		return ""
	}
	return parsed.Path
}

// Query returns the raw request query.
func (r RequestHead) Query() (string, bool) {
	parsed, err := url.ParseRequestURI(r.Target)
	if err != nil || parsed.RawQuery == "" {
		return "", false
	}
	return parsed.RawQuery, true
}

// CallbackURL reconstructs the callback URL with its original query encoding.
func (r RequestHead) CallbackURL(authority string) string { return "http://" + authority + r.Target }

// QueryValue returns the first decoded value for one query key.
func QueryValue(raw, key string) (string, bool) {
	values, err := url.ParseQuery(raw)
	if err != nil || len(values[key]) == 0 {
		return "", false
	}
	return values[key][0], true
}

const (
	callbackPath     = "/oauth/callback"
	maxCallbackBytes = 8 * 1024
)

// ConnectionStage is the safe preview of a pending remote connection.
type ConnectionStage struct{ Origin string }

// PendingConnection is a validated remote connection request.
type PendingConnection struct{ origin string }

// ParsePendingConnection accepts a direct HTTPS origin or a noema connect link.
func ParsePendingConnection(raw string) (PendingConnection, error) {
	raw = strings.TrimSpace(raw)
	if strings.HasPrefix(raw, "https://") {
		origin, err := TrustedOrigin(raw)
		return PendingConnection{origin: origin}, err
	}
	parsed, err := url.Parse(raw)
	if err != nil || parsed.Scheme != "noema" || parsed.Host != "connect" || (parsed.Path != "" && parsed.Path != "/") || parsed.Fragment != "" {
		return PendingConnection{}, errors.New("Enter a valid Noema server address.")
	}
	values, err := url.ParseQuery(parsed.RawQuery)
	if err != nil || len(values) != 1 || len(values["origin"]) != 1 {
		return PendingConnection{}, errors.New("The Noema connection link is invalid.")
	}
	origin, err := TrustedOrigin(values["origin"][0])
	if err != nil {
		return PendingConnection{}, err
	}
	return PendingConnection{origin: origin}, nil
}

// Stage returns the validated connection origin.
func (p PendingConnection) Stage() ConnectionStage { return ConnectionStage{Origin: p.origin} }

// TrustedOrigin validates one public HTTPS origin.
func TrustedOrigin(raw string) (string, error) {
	parsed, err := url.Parse(raw)
	if err != nil || parsed.Scheme != "https" || parsed.Host == "" || parsed.User != nil || parsed.RawQuery != "" || parsed.ForceQuery || parsed.Fragment != "" || (parsed.Path != "" && parsed.Path != "/") {
		return "", errors.New("Connection requires a trusted HTTPS server origin.")
	}
	host := parsed.Hostname()
	local := strings.EqualFold(host, "localhost") || strings.HasSuffix(strings.ToLower(host), ".localhost") || net.ParseIP(host) != nil
	if local {
		return "", errors.New("Connection requires a trusted HTTPS server origin.")
	}
	return "https://" + strings.ToLower(parsed.Host), nil
}

type callbackResult struct {
	code   string
	state  string
	denied bool
}

// ReceiveCallback accepts one loopback OAuth callback and writes the safe
// browser handoff page before returning the authorization code.
func ReceiveCallback(listener net.Listener, redirect, origin, expectedState string) (string, error) {
	connection, err := listener.Accept()
	if err != nil {
		return "", errors.New("Noema could not receive the sign-in response.")
	}
	defer connection.Close()
	if address, ok := connection.RemoteAddr().(*net.TCPAddr); !ok || !address.IP.IsLoopback() {
		return "", errors.New("Noema rejected a non-local sign-in response.")
	}
	response, readErr := readCallback(connection, redirect)
	denied := readErr == nil && response.denied && response.state == expectedState
	var result string
	if readErr == nil && !response.denied && response.state == expectedState {
		result = response.code
	}
	resultErr := error(nil)
	if result == "" {
		resultErr = errors.New("Noema did not complete this sign-in. Try again.")
	}
	title, intro, note, status := "Sign-in did not finish", "Return to Noema Desktop to try again.", "Start a new connection from the app.", "400 Bad Request"
	if denied {
		title, intro, note, status = "Connection declined", "No new access was granted.", "Return to Noema Desktop when you’re ready.", "200 OK"
	} else if resultErr == nil {
		title, intro, note, status = "Continue in Noema Desktop", "The app will finish connecting.", "If the app did not open, return to it now.", "200 OK"
	}
	page := fmt.Sprintf("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"referrer\" content=\"no-referrer\"><title>%s · Noema</title><link rel=\"stylesheet\" href=\"%s/assets/supporting.css\"></head><body><main><section><h1>%s</h1><p>%s</p><p>%s</p></section></main></body></html>", title, origin, title, intro, note)
	_, _ = fmt.Fprintf(connection, "HTTP/1.1 %s\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: %d\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\r\n%s", status, len([]byte(page)), page)
	if resultErr != nil {
		return "", resultErr
	}
	return result, nil
}

func readCallback(connection net.Conn, redirect string) (callbackResult, error) {
	target, err := readCallbackTarget(connection)
	if err != nil {
		return callbackResult{}, err
	}
	base, err := url.Parse(redirect)
	if err != nil {
		return callbackResult{}, errors.New("Noema received an invalid sign-in response.")
	}
	callback, err := base.Parse(target)
	if err != nil || callback.Scheme != base.Scheme || callback.Host != base.Host || callback.Path != callbackPath || callback.Fragment != "" {
		return callbackResult{}, errors.New("Noema received an invalid sign-in response.")
	}
	values, err := url.ParseQuery(callback.RawQuery)
	if err != nil || len(values) != 2 {
		return callbackResult{}, errors.New("Noema received an invalid sign-in response.")
	}
	for _, entries := range values {
		if len(entries) != 1 {
			return callbackResult{}, errors.New("Noema received an invalid sign-in response.")
		}
	}
	state := values.Get("state")
	if len(state) < 1 || len(state) > 256 {
		return callbackResult{}, errors.New("Noema received an invalid sign-in response.")
	}
	if values.Get("error") == "access_denied" {
		return callbackResult{state: state, denied: true}, nil
	}
	code := values.Get("code")
	if len(code) < 1 || len(code) > 128 {
		return callbackResult{}, errors.New("Noema received an invalid sign-in response.")
	}
	return callbackResult{code: code, state: state}, nil
}

func readCallbackTarget(connection net.Conn) (string, error) {
	var bytesRead []byte
	buffer := make([]byte, 1024)
	for {
		count, err := connection.Read(buffer)
		if count == 0 {
			if err == nil {
				continue
			}
			return "", errors.New("Noema received an invalid sign-in response.")
		}
		if len(bytesRead)+count > maxCallbackBytes {
			return "", errors.New("Noema received an invalid sign-in response.")
		}
		bytesRead = append(bytesRead, buffer[:count]...)
		end := bytes.Index(bytesRead, []byte("\r\n\r\n"))
		if end < 0 {
			continue
		}
		lineEnd := bytes.Index(bytesRead[:end], []byte("\r\n"))
		if lineEnd < 0 {
			return "", errors.New("Noema received an invalid sign-in response.")
		}
		fields := strings.Fields(string(bytesRead[:lineEnd]))
		if len(fields) != 3 || fields[0] != http.MethodGet || fields[2] != "HTTP/1.1" {
			return "", errors.New("Noema received an invalid sign-in response.")
		}
		return fields[1], nil
	}
}
