// Package auth owns browser authentication and public HTTP admission.
package auth

import (
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/base64"
	"errors"
	"fmt"
	"net"
	"net/url"
	"os"
	"runtime"
	"strconv"
	"strings"
	"sync"

	"github.com/goccy/go-yaml"
	"github.com/kpsuperplane/noema/internal/home"
)

const configLimit = 1024 * 1024

// Config contains non-secret browser authentication configuration.
type Config struct {
	DomainSetupRequired  bool
	Authority            string
	Origin               string
	RPID                 string
	ListenAddress        string
	Secure               bool
	DevNoAuth            bool
	GraphiQL             bool
	LocalGraphQLSocket   bool
	BrowserMaxSessions   int
	BrowserMaxOldSpaceMB int
}

// Recovery owns serialized one-time recovery-code rotation.
type Recovery struct {
	path     string
	mu       sync.Mutex
	disabled bool
}

// recoveryCode keeps decoded recovery material out of ordinary debug output.
type recoveryCode struct {
	value []byte
}

func (recoveryCode) GoString() string { return "RecoveryCode{value: [REDACTED]}" }

// RecoveryErrorKind identifies the recovery-code state that prevented an
// attempt from completing.
type RecoveryErrorKind string

const (
	RecoveryMissingField   RecoveryErrorKind = "missing_field"
	RecoveryMalformedField RecoveryErrorKind = "malformed_field"
	RecoveryRead           RecoveryErrorKind = "read"
	RecoveryRandom         RecoveryErrorKind = "random"
	RecoveryVerification   RecoveryErrorKind = "verification"
	RecoveryUnavailable    RecoveryErrorKind = "unavailable"
)

// RecoveryError preserves the recovery authority's failure category while
// keeping the underlying filesystem failure available to callers.
type RecoveryError struct {
	Kind  RecoveryErrorKind
	Cause error
}

func (e *RecoveryError) Error() string {
	switch e.Kind {
	case RecoveryMissingField:
		return "web.recovery_code is missing"
	case RecoveryMalformedField:
		return "web.recovery_code must be canonical unpadded base64url for exactly 32 bytes"
	case RecoveryRead:
		return "failed to read recovery configuration"
	case RecoveryRandom:
		return "failed to generate a recovery code"
	case RecoveryVerification:
		return "failed to verify the committed recovery code"
	case RecoveryUnavailable:
		return "recovery is unavailable until Noema restarts"
	default:
		return "recovery failed"
	}
}

func (e *RecoveryError) Unwrap() error { return e.Cause }

// NewRecovery opens the recovery-code authority for an already loaded host
// configuration. It does not read or rewrite the configuration file.
func NewRecovery(paths home.Paths) *Recovery {
	return &Recovery{path: paths.Config()}
}

// LoadConfig loads public settings and ensures one protected recovery code.
func LoadConfig(paths home.Paths, listenAddress string) (Config, *Recovery, error) {
	return loadConfig(paths, listenAddress, true)
}

// LoadWebConfig loads public settings without changing a first-run config
// document. The recovery authority can be opened against the same path.
func LoadWebConfig(paths home.Paths, listenAddress string) (Config, *Recovery, error) {
	return loadConfig(paths, listenAddress, false)
}

func loadConfig(paths home.Paths, listenAddress string, ensureRecovery bool) (Config, *Recovery, error) {
	document, err := readConfigDocument(paths.Config())
	if err != nil {
		return Config{}, nil, err
	}
	web, err := childMap(document, "web")
	if err != nil {
		return Config{}, nil, err
	}
	browser := map[string]any{}
	if raw, exists := document["browser"]; exists {
		var ok bool
		browser, ok = raw.(map[string]any)
		if !ok {
			return Config{}, nil, errors.New("config.yaml field browser must be a mapping")
		}
	}
	browserMaxSessions, err := configBoundedInt(browser, "max_sessions", 2, 1, 8, "browser.max_sessions", paths.Config())
	if err != nil {
		return Config{}, nil, err
	}
	browserOldSpace, err := configBoundedInt(browser, "max_old_space_mb", 1024, 256, 4096, "browser.max_old_space_mb", paths.Config())
	if err != nil {
		return Config{}, nil, err
	}
	for name, value := range map[string]*int{"NOEMA_BROWSER__MAX_SESSIONS": &browserMaxSessions, "NOEMA_BROWSER__MAX_OLD_SPACE_MB": &browserOldSpace} {
		if raw, exists := os.LookupEnv(name); exists {
			parsed, parseErr := strconv.Atoi(raw)
			minimum, maximum := 1, 8
			if name == "NOEMA_BROWSER__MAX_OLD_SPACE_MB" {
				minimum, maximum = 256, 4096
			}
			if parseErr != nil || parsed < minimum || parsed > maximum {
				return Config{}, nil, &ProviderConfigError{Kind: ProviderConfigInvalidNumber, Path: paths.Config(), Name: name, Value: raw}
			}
			*value = parsed
		}
	}
	recoveryValue, recoveryExists := web["recovery_code"]
	recoveryCode, recoveryIsString := recoveryValue.(string)
	if !recoveryExists && ensureRecovery {
		recoveryCode, err = randomBase64URL(32)
		if err != nil {
			return Config{}, nil, err
		}
		web["recovery_code"] = recoveryCode
		if err := writeConfigDocument(paths.Config(), document); err != nil {
			return Config{}, nil, fmt.Errorf("store recovery code: %w", err)
		}
	} else if recoveryExists && (!recoveryIsString || !validRecoveryCode(recoveryCode)) {
		return Config{}, nil, &RecoveryError{Kind: RecoveryMalformedField}
	}

	host, err := configString(web, "host", "127.0.0.1", false)
	if err != nil {
		return Config{}, nil, err
	}
	port, err := configPort(web, "port", 3737)
	if err != nil {
		return Config{}, nil, err
	}
	if value, exists := os.LookupEnv("NOEMA_WEB__HOST"); exists {
		host = value
	}
	if value, exists := os.LookupEnv("NOEMA_WEB__PORT"); exists {
		parsed, parseErr := strconv.ParseUint(value, 10, 16)
		if parseErr != nil || parsed == 0 {
			return Config{}, nil, errors.New("NOEMA_WEB__PORT must be an integer from 1 through 65535")
		}
		port = uint16(parsed)
	}
	if net.ParseIP(host) == nil {
		return Config{}, nil, errors.New("web.host must be a numeric IP address")
	}
	resolvedAddress := net.JoinHostPort(host, strconv.Itoa(int(port)))
	if listenAddress != "" {
		resolvedAddress = listenAddress
	}
	_, portText, err := net.SplitHostPort(resolvedAddress)
	if err != nil {
		return Config{}, nil, errors.New("listen address must contain a host and port")
	}
	if portText == "0" {
		return Config{}, nil, errors.New("listen port must be fixed before authentication starts")
	}
	publicOrigin, err := configString(web, "public_origin", "http://localhost:"+portText, true)
	if err != nil {
		return Config{}, nil, err
	}
	rpID, err := configString(web, "rp_id", "localhost", false)
	if err != nil {
		return Config{}, nil, err
	}
	devNoAuth, err := configBool(web, "dev_no_auth", false)
	if err != nil {
		return Config{}, nil, err
	}
	if value, exists := os.LookupEnv("NOEMA_WEB__PUBLIC_ORIGIN"); exists {
		publicOrigin = value
	}
	if value, exists := os.LookupEnv("NOEMA_WEB__RP_ID"); exists {
		rpID = value
	}
	if value, exists := os.LookupEnv("NOEMA_WEB__DEV_NO_AUTH"); exists {
		parsed, parseErr := strconv.ParseBool(value)
		if parseErr != nil {
			return Config{}, nil, errors.New("NOEMA_WEB__DEV_NO_AUTH must be true or false")
		}
		devNoAuth = parsed
	}
	graphiQL, err := configBool(web, "graphiql", false)
	if err != nil {
		return Config{}, nil, err
	}
	localSocket, err := configBool(web, "local_graphql_socket", runtime.GOOS != "windows")
	if err != nil {
		return Config{}, nil, err
	}
	for name, target := range map[string]*bool{
		"NOEMA_WEB__GRAPHIQL":             &graphiQL,
		"NOEMA_WEB__LOCAL_GRAPHQL_SOCKET": &localSocket,
	} {
		if value, exists := os.LookupEnv(name); exists {
			parsed, parseErr := strconv.ParseBool(value)
			if parseErr != nil {
				return Config{}, nil, fmt.Errorf("%s must be true or false", name)
			}
			*target = parsed
		}
	}
	config, err := canonicalConfig(publicOrigin, rpID, devNoAuth)
	if err != nil {
		return Config{}, nil, err
	}
	_, originFromEnvironment := os.LookupEnv("NOEMA_WEB__PUBLIC_ORIGIN")
	config.DomainSetupRequired = web["public_origin"] == nil && !originFromEnvironment && !devNoAuth
	config.ListenAddress = resolvedAddress
	config.GraphiQL = graphiQL
	config.LocalGraphQLSocket = localSocket
	config.BrowserMaxSessions = browserMaxSessions
	config.BrowserMaxOldSpaceMB = browserOldSpace
	return config, &Recovery{path: paths.Config()}, nil
}

func configBoundedInt(values map[string]any, key string, fallback, minimum, maximum int, field, path string) (int, error) {
	raw, exists := values[key]
	if !exists {
		return fallback, nil
	}
	value, ok := raw.(int)
	if !ok {
		if unsigned, valid := raw.(uint64); valid && unsigned <= uint64(maximum) {
			value, ok = int(unsigned), true
		}
	}
	if !ok || value < minimum || value > maximum {
		return 0, &ProviderConfigError{Kind: ProviderConfigInvalidNumber, Path: path, Name: field, Value: fmt.Sprint(raw)}
	}
	return value, nil
}

// Attempt rotates the code before it reports whether one candidate matched.
func (r *Recovery) Attempt(candidate string) (bool, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.disabled {
		return false, &RecoveryError{Kind: RecoveryUnavailable}
	}
	if _, err := os.Stat(r.path); err != nil {
		r.disabled = true
		return false, &RecoveryError{Kind: RecoveryRead, Cause: err}
	}
	document, err := readConfigDocument(r.path)
	if err != nil {
		r.disabled = true
		return false, &RecoveryError{Kind: RecoveryRead, Cause: err}
	}
	web, err := childMap(document, "web")
	if err != nil {
		r.disabled = true
		return false, &RecoveryError{Kind: RecoveryRead, Cause: err}
	}
	current, _ := web["recovery_code"].(string)
	if !validRecoveryCode(current) {
		r.disabled = true
		return false, &RecoveryError{Kind: RecoveryMissingField}
	}
	want := sha256.Sum256([]byte(current))
	got := sha256.Sum256([]byte(candidate))
	matched := subtle.ConstantTimeCompare(want[:], got[:]) == 1
	replacement, err := randomBase64URL(32)
	if err != nil {
		r.disabled = true
		return false, &RecoveryError{Kind: RecoveryRandom, Cause: err}
	}
	web["recovery_code"] = replacement
	if err := writeConfigDocument(r.path, document); err != nil {
		r.disabled = true
		return false, &RecoveryError{Kind: RecoveryVerification, Cause: err}
	}
	verified, err := readConfigDocument(r.path)
	if err != nil {
		r.disabled = true
		return false, &RecoveryError{Kind: RecoveryVerification, Cause: err}
	}
	verifiedWeb, err := childMap(verified, "web")
	verifiedCode, codeErr := configString(verifiedWeb, "recovery_code", "", false)
	if err != nil || codeErr != nil || verifiedCode != replacement {
		r.disabled = true
		return false, &RecoveryError{Kind: RecoveryVerification}
	}
	return matched, nil
}

func canonicalConfig(publicOrigin string, rpID string, devNoAuth bool) (Config, error) {
	parsed, err := url.Parse(publicOrigin)
	if err != nil || parsed.Hostname() == "" {
		return Config{}, errors.New("web.public_origin must be a URL with a host")
	}
	host := strings.ToLower(parsed.Hostname())
	secure := parsed.Scheme == "https"
	if !secure && (parsed.Scheme != "http" || host != "localhost") {
		return Config{}, errors.New("web.public_origin must use HTTPS, except for localhost")
	}
	if parsed.User != nil || parsed.Path != "" && parsed.Path != "/" ||
		parsed.RawQuery != "" || parsed.Fragment != "" {
		return Config{}, errors.New("web.public_origin must contain only scheme, host, and optional port")
	}
	if host != "localhost" && net.ParseIP(host) != nil {
		return Config{}, errors.New("web.public_origin must use a domain name for passkeys")
	}
	if strings.TrimSpace(rpID) != rpID || rpID == "" || strings.ContainsAny(rpID, "/:") ||
		(rpID != "localhost" && net.ParseIP(rpID) != nil) {
		return Config{}, errors.New("web.rp_id must be a domain name without a scheme or port")
	}
	rpID = strings.ToLower(rpID)
	if host != rpID && !strings.HasSuffix(host, "."+rpID) {
		return Config{}, errors.New("web.rp_id must equal or be a parent domain of the public origin host")
	}
	authority := host
	if port := parsed.Port(); port != "" {
		authority = net.JoinHostPort(host, port)
	}
	return Config{
		Authority: authority,
		Origin:    parsed.Scheme + "://" + authority,
		RPID:      rpID,
		Secure:    secure,
		DevNoAuth: devNoAuth,
	}, nil
}

func readConfigDocument(path string) (map[string]any, error) {
	data, err := home.ReadPrivateFile(path, configLimit)
	if errors.Is(err, os.ErrNotExist) {
		return make(map[string]any), nil
	}
	if err != nil {
		return nil, fmt.Errorf("read config.yaml: %w", err)
	}
	document := make(map[string]any)
	if len(data) == 0 {
		return document, nil
	}
	var value any
	if err := yaml.Unmarshal(data, &value); err != nil {
		return nil, errors.New("config.yaml is not valid YAML")
	}
	if value == nil {
		return nil, errors.New("config.yaml must contain a mapping")
	}
	var ok bool
	if document, ok = value.(map[string]any); !ok {
		return nil, errors.New("config.yaml must contain a mapping")
	}
	return document, nil
}

func writeConfigDocument(path string, document map[string]any) error {
	data, err := yaml.Marshal(document)
	if err != nil {
		return errors.New("encode config.yaml")
	}
	if int64(len(data)) > configLimit {
		return errors.New("config.yaml exceeds its size limit")
	}
	return home.AtomicWritePrivate(path, data)
}

func childMap(document map[string]any, key string) (map[string]any, error) {
	if raw, exists := document[key]; exists {
		value, ok := raw.(map[string]any)
		if !ok {
			return nil, fmt.Errorf("config.yaml field %s must be a mapping", key)
		}
		return value, nil
	}
	value := make(map[string]any)
	document[key] = value
	return value, nil
}

func configString(values map[string]any, key string, fallback string, allowNull bool) (string, error) {
	raw, exists := values[key]
	if !exists || allowNull && raw == nil {
		return fallback, nil
	}
	value, ok := raw.(string)
	if !ok {
		return "", fmt.Errorf("config.yaml field web.%s must be a string", key)
	}
	return value, nil
}

func configBool(values map[string]any, key string, fallback bool) (bool, error) {
	raw, exists := values[key]
	if !exists {
		return fallback, nil
	}
	value, ok := raw.(bool)
	if !ok {
		return false, fmt.Errorf("config.yaml field web.%s must be true or false", key)
	}
	return value, nil
}

func configPort(values map[string]any, key string, fallback uint16) (uint16, error) {
	raw, exists := values[key]
	if !exists {
		return fallback, nil
	}
	var value uint64
	switch number := raw.(type) {
	case uint64:
		value = number
	case int:
		if number > 0 {
			value = uint64(number)
		}
	}
	if value == 0 || value > 65535 {
		return 0, fmt.Errorf("config.yaml field web.%s must be an integer from 1 through 65535", key)
	}
	return uint16(value), nil
}

func validRecoveryCode(value string) bool {
	_, err := parseRecoveryCode(value)
	return err == nil
}

func parseRecoveryCode(value string) (recoveryCode, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(value)
	if err != nil || len(decoded) != 32 || base64.RawURLEncoding.EncodeToString(decoded) != value {
		return recoveryCode{}, errors.New("recovery code is not canonical")
	}
	return recoveryCode{value: decoded}, nil
}

func randomBase64URL(size int) (string, error) {
	value := make([]byte, size)
	if _, err := rand.Read(value); err != nil {
		return "", fmt.Errorf("generate random value: %w", err)
	}
	return base64.RawURLEncoding.EncodeToString(value), nil
}
