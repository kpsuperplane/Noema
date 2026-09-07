package adapter

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"net/url"
	"os"
	"reflect"
	"sort"
	"strings"

	"github.com/kpsuperplane/noema/internal/home"
)

const oauthObjectLimit = 128 << 10

// OAuthIntegrityError identifies a filesystem integrity category without
// exposing object contents.
type OAuthIntegrityError struct{ Code string }

func (e *OAuthIntegrityError) Error() string {
	if e == nil {
		return "adapter OAuth authority integrity failed"
	}
	return "adapter OAuth authority integrity failed: " + e.Code
}

// OAuthProfile is one reviewed public OAuth protocol contract.
type OAuthProfile struct {
	SchemaVersion                   int               `json:"schema_version"`
	ProfileID                       string            `json:"profile_id"`
	DisplayName                     string            `json:"display_name"`
	AuthorizationEndpoint           string            `json:"authorization_endpoint"`
	TokenEndpoint                   string            `json:"token_endpoint"`
	ClientAuthentication            string            `json:"client_authentication"`
	Setups                          []OAuthSetup      `json:"setups"`
	AuthorizationParameters         map[string]string `json:"authorization_parameters"`
	AccountSelectionParameters      map[string]string `json:"account_selection_parameters"`
	GrantAudience                   string            `json:"grant_audience"`
	OmittedScopePolicy              string            `json:"omitted_scope_policy"`
	PreserveRefreshTokenOnExpansion bool              `json:"preserve_refresh_token_on_expansion"`
	ProfileDigest                   string            `json:"-"`
}

type OAuthSetup struct {
	CallbackMode string          `json:"callback_mode"`
	Setup        CredentialSetup `json:"setup"`
}
type OAuthApplication struct {
	SchemaVersion        int     `json:"schema_version"`
	ApplicationID        string  `json:"application_id"`
	ProfileDigest        string  `json:"profile_digest"`
	CallbackMode         string  `json:"callback_mode"`
	ClientID             string  `json:"client_id"`
	ProjectLabel         *string `json:"project_label,omitempty"`
	CredentialGeneration string  `json:"credential_generation"`
	Revision             int     `json:"revision"`
	Status               string  `json:"status"`
}
type oauthApplicationCredential struct {
	SchemaVersion int    `json:"schema_version"`
	GenerationID  string `json:"generation_id"`
	ClientSecret  string `json:"client_secret,omitempty"`
}
type ExternalAccount struct {
	SchemaVersion   int     `json:"schema_version"`
	AccountID       string  `json:"account_id"`
	ProfileDigest   string  `json:"profile_digest"`
	ProviderSubject string  `json:"provider_subject"`
	AccountLabel    *string `json:"account_label,omitempty"`
	Revision        int     `json:"revision"`
}
type OAuthGrant struct {
	SchemaVersion     int      `json:"schema_version"`
	GrantID           string   `json:"grant_id"`
	ApplicationID     string   `json:"application_id"`
	AccountID         *string  `json:"account_id,omitempty"`
	AccountLabel      *string  `json:"account_label,omitempty"`
	Audience          string   `json:"audience"`
	DesiredScopes     []string `json:"desired_scopes"`
	GrantedScopes     []string `json:"granted_scopes"`
	AuthorityRevision int      `json:"authority_revision"`
	TokenGeneration   *string  `json:"token_generation,omitempty"`
	TokenRevision     int      `json:"token_revision"`
	Status            string   `json:"status"`
}
type oauthGrantToken struct {
	SchemaVersion int      `json:"schema_version"`
	GenerationID  string   `json:"generation_id"`
	AccessToken   string   `json:"access_token"`
	RefreshToken  string   `json:"refresh_token,omitempty"`
	ExpiresAt     int64    `json:"expires_at_epoch_seconds,omitempty"`
	Scopes        []string `json:"-"`
}

// GoString prevents bearer and refresh material from entering diagnostics.
func (t oauthGrantToken) GoString() string {
	return fmt.Sprintf("adapter.oauthGrantToken{SchemaVersion:%d, GenerationID:%q, AccessToken:%q, RefreshToken:%q, ExpiresAt:%d, Scopes:%#v}", t.SchemaVersion, t.GenerationID, "[REDACTED]", "[REDACTED]", t.ExpiresAt, t.Scopes)
}

type OAuthSnapshot struct {
	Profiles     []OAuthProfile
	Applications []OAuthApplication
	Accounts     []ExternalAccount
	Grants       []OAuthGrant
}

func googleOAuthProfile() OAuthProfile {
	fields := func(redirects bool) []CredentialField {
		value := []CredentialField{{ID: "client_id", Label: "Client ID"}, {ID: "client_secret", Label: "Client secret"}}
		if redirects {
			value = append(value, CredentialField{ID: "redirect_uris", Label: "Registered redirect URIs"})
		}
		return value
	}
	p := OAuthProfile{SchemaVersion: 1, ProfileID: "google", DisplayName: "Google", AuthorizationEndpoint: "https://accounts.google.com/o/oauth2/v2/auth", TokenEndpoint: "https://oauth2.googleapis.com/token", ClientAuthentication: "client_secret_post",
		AuthorizationParameters: map[string]string{"access_type": "offline", "include_granted_scopes": "true"}, AccountSelectionParameters: map[string]string{"prompt": "select_account"}, GrantAudience: "google-apis", OmittedScopePolicy: "requested_scopes", PreserveRefreshTokenOnExpansion: true}
	p.Setups = []OAuthSetup{
		{CallbackMode: "loopback", Setup: CredentialSetup{CredentialType: "Desktop app OAuth client", SetupURL: "https://console.cloud.google.com/apis/credentials", Instructions: []string{"Create one Desktop app OAuth client.", "Download its JSON document."}, Input: CredentialInput{Kind: "document", MediaType: "application/json", Fields: fields(false), Normalize: &Transform{Language: "lua", Source: "return function(input) local d = json.decode(input.document) return { client_id = d.installed.client_id, client_secret = d.installed.client_secret } end"}}}},
		{CallbackMode: "hosted", Setup: CredentialSetup{CredentialType: "Web application OAuth client", SetupURL: "https://console.cloud.google.com/apis/credentials", Instructions: []string{"Create one Web application OAuth client.", "Add the shown redirect URI.", "Download its JSON document."}, Input: CredentialInput{Kind: "document", MediaType: "application/json", Fields: fields(true), Normalize: &Transform{Language: "lua", Source: "return function(input) local d = json.decode(input.document) return { client_id = d.web.client_id, client_secret = d.web.client_secret, redirect_uris = json.encode(d.web.redirect_uris) } end"}}}},
	}
	raw, _ := json.Marshal(p)
	var canonical any
	_ = json.Unmarshal(raw, &canonical)
	raw, _ = json.Marshal(canonical)
	p.ProfileDigest = sha256Hex(raw)
	return p
}

func validateOAuthProfile(p OAuthProfile) error {
	if p.SchemaVersion != 1 || !validID(p.ProfileID) || !boundedText(p.DisplayName, 256, false) ||
		!boundedText(p.GrantAudience, 256, false) || p.ClientAuthentication != "none" && p.ClientAuthentication != "client_secret_basic" && p.ClientAuthentication != "client_secret_post" ||
		p.OmittedScopePolicy != "requested_scopes" || !p.PreserveRefreshTokenOnExpansion || len(p.Setups) == 0 || len(p.Setups) > 2 {
		return errors.New("adapter OAuth profile uses an unsupported contract")
	}
	for _, endpoint := range []string{p.AuthorizationEndpoint, p.TokenEndpoint} {
		u, err := url.Parse(endpoint)
		if err != nil || len(endpoint) > 4096 || u.Scheme != "https" || u.Hostname() == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" {
			return errors.New("adapter OAuth profile endpoint is invalid")
		}
	}
	reserved := map[string]bool{"client_id": true, "client_secret": true, "response_type": true, "redirect_uri": true, "scope": true, "state": true, "code_challenge": true, "code_challenge_method": true}
	for _, parameters := range []map[string]string{p.AuthorizationParameters, p.AccountSelectionParameters} {
		if len(parameters) > 16 {
			return errors.New("adapter OAuth profile parameters are invalid")
		}
		for name, value := range parameters {
			if reserved[name] || !boundedText(name, 128, false) || !boundedText(value, 512, false) {
				return errors.New("adapter OAuth profile parameter is invalid")
			}
		}
	}
	modes := map[string]bool{}
	for _, entry := range p.Setups {
		if modes[entry.CallbackMode] || entry.CallbackMode != "hosted" && entry.CallbackMode != "loopback" ||
			entry.Setup.Input.Kind != "document" {
			return errors.New("adapter OAuth profile setup is invalid")
		}
		modes[entry.CallbackMode] = true
		auth := Authentication{Kind: "credential", Setup: &entry.Setup, RequestAuth: &Transform{Language: "lua", Source: "return function(input) return {} end"}}
		if validateAuthentication(auth) != nil {
			return errors.New("adapter OAuth profile setup is invalid")
		}
		fields := map[string]bool{}
		for _, field := range entry.Setup.Input.Fields {
			fields[field.ID] = true
		}
		wanted := 2
		if entry.CallbackMode == "hosted" {
			wanted = 3
		}
		if len(fields) != wanted || !fields["client_id"] || !fields["client_secret"] || entry.CallbackMode == "hosted" && !fields["redirect_uris"] {
			return errors.New("adapter OAuth profile credential fields are invalid")
		}
	}
	return nil
}

func (f *fileAuthority) installOAuthProfile(profile OAuthProfile) error {
	if err := validateOAuthProfile(profile); err != nil {
		return err
	}
	raw, _ := json.Marshal(profile)
	path := "adapters/oauth-profiles/" + profile.ProfileDigest
	if _, err := f.root.Lstat(path); err == nil {
		existing, e := f.loadOAuthProfile(profile.ProfileDigest)
		if e != nil || existing.ProfileDigest != profile.ProfileDigest {
			return errors.New("adapter OAuth profile changed")
		}
		return nil
	}
	return f.installDirectory("adapters/oauth-profiles", profile.ProfileDigest, map[string][]byte{"profile.json": raw})
}

func (f *fileAuthority) loadOAuthProfile(digest string) (OAuthProfile, error) {
	if !validDigest(digest) {
		return OAuthProfile{}, errors.New("adapter OAuth profile is unavailable")
	}
	directory, err := f.root.OpenRoot("adapters/oauth-profiles/" + digest)
	if err != nil {
		return OAuthProfile{}, errors.New("adapter OAuth profile is unavailable")
	}
	defer directory.Close()
	if err := exactFiles(directory, []string{"profile.json"}); err != nil {
		return OAuthProfile{}, &OAuthIntegrityError{Code: "object_entries"}
	}
	raw, err := readRegular(directory, "profile.json", oauthObjectLimit)
	if err != nil {
		return OAuthProfile{}, err
	}
	var p OAuthProfile
	if decodeExactJSON(raw, &p) != nil {
		return OAuthProfile{}, errors.New("adapter OAuth profile is invalid")
	}
	encoded, _ := json.Marshal(p)
	var value any
	_ = json.Unmarshal(encoded, &value)
	encoded, _ = json.Marshal(value)
	p.ProfileDigest = sha256Hex(encoded)
	if p.ProfileDigest != digest || validateOAuthProfile(p) != nil {
		return OAuthProfile{}, errors.New("adapter OAuth profile changed")
	}
	return p, nil
}

func readObject(root *os.Root, directory, name string) ([]byte, error) {
	d, err := root.OpenRoot(directory)
	if err != nil {
		return nil, errors.New("adapter OAuth object is unavailable")
	}
	defer d.Close()
	return readRegular(d, name, oauthObjectLimit)
}

func (f *fileAuthority) oauthSnapshot() (OAuthSnapshot, error) {
	entries, err := readBoundedEntries(f.root, "adapters/oauth-profiles", definitionLimit)
	if err != nil {
		return OAuthSnapshot{}, err
	}
	out := OAuthSnapshot{Profiles: []OAuthProfile{}}
	for _, entry := range entries {
		if !entry.IsDir() || !validDigest(entry.Name()) {
			continue
		}
		p, err := f.loadOAuthProfile(entry.Name())
		if err != nil {
			return OAuthSnapshot{}, err
		}
		out.Profiles = append(out.Profiles, p)
	}
	if err = loadOAuthObjects(f, "adapters/oauth-applications", "application.json", func(raw []byte) error {
		var v OAuthApplication
		if decodeExactJSON(raw, &v) != nil || !validOAuthApplication(v) {
			return errors.New("invalid")
		}
		out.Applications = append(out.Applications, v)
		return nil
	}); err != nil {
		return OAuthSnapshot{}, err
	}
	if err = loadOAuthObjects(f, "adapters/oauth-accounts", "account.json", func(raw []byte) error {
		var v ExternalAccount
		if decodeExactJSON(raw, &v) != nil || !validConnectionID(v.AccountID) || v.SchemaVersion != 1 {
			return errors.New("invalid")
		}
		out.Accounts = append(out.Accounts, v)
		return nil
	}); err != nil {
		return OAuthSnapshot{}, err
	}
	if err = loadOAuthObjects(f, "adapters/oauth-grants", "grant.json", func(raw []byte) error {
		var v OAuthGrant
		if decodeExactJSON(raw, &v) != nil || !validOAuthGrant(v) {
			return errors.New("invalid")
		}
		out.Grants = append(out.Grants, v)
		return nil
	}); err != nil {
		return OAuthSnapshot{}, err
	}
	sort.Slice(out.Applications, func(i, j int) bool { return out.Applications[i].ApplicationID < out.Applications[j].ApplicationID })
	sort.Slice(out.Grants, func(i, j int) bool { return out.Grants[i].GrantID < out.Grants[j].GrantID })
	return out, nil
}
func loadOAuthObjects(f *fileAuthority, path, file string, accept func([]byte) error) error {
	entries, err := readBoundedEntries(f.root, path, 1024)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".") || !entry.IsDir() || !validConnectionID(entry.Name()) {
			continue
		}
		raw, e := readObject(f.root, path+"/"+entry.Name(), file)
		if e != nil {
			return e
		}
		if e = accept(raw); e != nil {
			return errors.New("adapter OAuth object is invalid")
		}
	}
	return nil
}
func validOAuthApplication(v OAuthApplication) bool {
	return v.SchemaVersion == 1 && validConnectionID(v.ApplicationID) && validDigest(v.ProfileDigest) && validConnectionID(v.CredentialGeneration) && v.ClientID != "" && len(v.ClientID) <= 16<<10 && (v.ProjectLabel == nil || boundedText(*v.ProjectLabel, 256, false)) && (v.CallbackMode == "hosted" || v.CallbackMode == "loopback") && v.Revision > 0 && (v.Status == "active" || v.Status == "suspended")
}
func validOAuthGrant(v OAuthGrant) bool {
	return v.SchemaVersion == 1 && validConnectionID(v.GrantID) && validConnectionID(v.ApplicationID) && v.Audience != "" && v.AuthorityRevision > 0 && v.TokenRevision > 0 && sortedUniqueScopes(v.DesiredScopes) && sortedUniqueScopes(v.GrantedScopes) && (v.Status == "active" || v.Status == "authentication_required" || v.Status == "revoked" || v.Status == "blocked")
}

func (f *fileAuthority) installOAuthApplication(v OAuthApplication, c oauthApplicationCredential) (OAuthApplication, error) {
	return v, f.installOAuthOwned("adapters/oauth-applications", v.ApplicationID, "application.json", v, "credentials", c.GenerationID, c)
}
func (f *fileAuthority) loadOAuthApplication(id string) (OAuthApplication, oauthApplicationCredential, error) {
	raw, e := readObject(f.root, "adapters/oauth-applications/"+id, "application.json")
	var v OAuthApplication
	if e != nil || decodeExactJSON(raw, &v) != nil || !validOAuthApplication(v) {
		return v, oauthApplicationCredential{}, errors.New("adapter OAuth application is unavailable")
	}
	secret, e := readObject(f.root, "adapters/oauth-applications/"+id+"/credentials", v.CredentialGeneration+".json")
	var c oauthApplicationCredential
	if e != nil || decodeExactJSON(secret, &c) != nil || c.GenerationID != v.CredentialGeneration {
		return v, c, errors.New("adapter OAuth application credential is unavailable")
	}
	return v, c, nil
}
func (f *fileAuthority) loadOAuthGrant(id string) (OAuthGrant, oauthGrantToken, error) {
	raw, e := readObject(f.root, "adapters/oauth-grants/"+id, "grant.json")
	var v OAuthGrant
	if e != nil || decodeExactJSON(raw, &v) != nil || !validOAuthGrant(v) {
		return v, oauthGrantToken{}, errors.New("adapter OAuth grant is unavailable")
	}
	if v.TokenGeneration == nil {
		return v, oauthGrantToken{}, nil
	}
	secret, e := readObject(f.root, "adapters/oauth-grants/"+id+"/tokens", *v.TokenGeneration+".json")
	var t oauthGrantToken
	if e != nil || decodeExactJSON(secret, &t) != nil || t.GenerationID != *v.TokenGeneration {
		return v, t, errors.New("adapter OAuth token is unavailable")
	}
	t.Scopes = append([]string(nil), v.GrantedScopes...)
	return v, t, nil
}

func (f *fileAuthority) replaceOAuthObject(parent, id, descriptor string, value any, secretDir, secretID string, secret any) error {
	if parent == "adapters/oauth-grants" && descriptor == "grant.json" {
		if _, ok := value.(OAuthGrant); !ok {
			return errors.New("adapter OAuth grant authority is invalid")
		}
	}
	d, err := f.root.OpenRoot(parent + "/" + id)
	if err != nil {
		return errors.New("adapter OAuth object is unavailable")
	}
	defer d.Close()
	if secretDir != "" {
		if err = d.Mkdir(secretDir, 0o700); err != nil && !errors.Is(err, fs.ErrExist) {
			return err
		}
		sr, e := d.OpenRoot(secretDir)
		if e != nil {
			return e
		}
		raw, _ := json.Marshal(secret)
		e = writeNewFile(sr, secretID+".json", raw)
		if e == nil {
			e = home.SyncRootDirectory(sr, ".")
		}
		_ = sr.Close()
		if e != nil {
			return errors.New("adapter OAuth secret publication failed")
		}
	}
	raw, _ := json.Marshal(value)
	tmp := "." + descriptor + "-" + randomHex()
	if err = writeNewFile(d, tmp, raw); err != nil {
		return err
	}
	if err = home.ReplaceRootFile(d, tmp, descriptor); err != nil {
		return err
	}
	if err = home.SyncRootDirectory(d, "."); err != nil {
		return err
	}
	if secretDir != "" {
		return pruneOAuthGenerations(d, secretDir, secretID)
	}
	return nil
}

func (f *fileAuthority) replaceOAuthGrantWithoutToken(value OAuthGrant) error {
	if err := f.replaceOAuthObject("adapters/oauth-grants", value.GrantID, "grant.json", value, "", "", nil); err != nil {
		return err
	}
	directory, err := f.root.OpenRoot("adapters/oauth-grants/" + value.GrantID)
	if err != nil {
		return err
	}
	defer directory.Close()
	if _, err = directory.Lstat("tokens"); err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return nil
		}
		return err
	}
	return pruneOAuthGenerations(directory, "tokens", "")
}

// deactivateOAuthGrant publishes the non-executable grant while retaining the
// previous grant directory in quarantine for diagnosis and recovery.
func (f *fileAuthority) deactivateOAuthGrant(value OAuthGrant) error {
	if value.GrantID == "" || value.TokenGeneration != nil {
		return errors.New("adapter OAuth grant deactivation is invalid")
	}
	raw, err := json.Marshal(value)
	if err != nil {
		return errors.New("adapter OAuth grant deactivation is invalid")
	}
	quarantine := "adapters/quarantine/oauth-grants/" + value.GrantID + "-" + randomHex()
	target := "adapters/oauth-grants/" + value.GrantID
	if err = f.root.Rename(target, quarantine); err != nil {
		return errors.New("adapter OAuth grant deactivation failed")
	}
	if err = f.installDirectory("adapters/oauth-grants", value.GrantID, map[string][]byte{"grant.json": raw}); err != nil {
		_ = f.root.Rename(quarantine, target)
		return errors.New("adapter OAuth grant deactivation failed")
	}
	if err = home.SyncRootDirectory(f.root, "adapters/quarantine/oauth-grants"); err != nil {
		return errors.New("adapter OAuth grant deactivation durability failed")
	}
	return nil
}

// replaceOAuthGrantGeneration applies the optimistic transition fence used by
// OAuth promotion and refresh. The descriptor and token publish together.
func (f *fileAuthority) replaceOAuthGrantGeneration(expected, replacement OAuthGrant, token oauthGrantToken, refresh bool) error {
	current, currentToken, err := f.loadOAuthGrant(expected.GrantID)
	if err != nil || !reflect.DeepEqual(current, expected) || current.SchemaVersion != replacement.SchemaVersion || current.GrantID != replacement.GrantID || current.ApplicationID != replacement.ApplicationID || !equalOptionalString(current.AccountID, replacement.AccountID) || current.Audience != replacement.Audience || replacement.TokenGeneration == nil || *replacement.TokenGeneration != token.GenerationID || current.TokenGeneration != nil && *current.TokenGeneration == token.GenerationID || replacement.TokenRevision != current.TokenRevision+1 || replacement.Status != "active" {
		return errors.New("adapter OAuth grant transition is invalid")
	}
	if refresh {
		if currentToken.GenerationID == "" || !reflect.DeepEqual(replacement.DesiredScopes, current.DesiredScopes) || !reflect.DeepEqual(replacement.GrantedScopes, current.GrantedScopes) || replacement.AuthorityRevision != current.AuthorityRevision {
			return errors.New("adapter OAuth grant transition is invalid")
		}
	} else if replacement.AuthorityRevision != current.AuthorityRevision+1 {
		return errors.New("adapter OAuth grant transition is invalid")
	}
	return f.replaceOAuthObject("adapters/oauth-grants", replacement.GrantID, "grant.json", replacement, "tokens", token.GenerationID, token)
}

func equalOptionalString(left, right *string) bool {
	if left == nil || right == nil {
		return left == right
	}
	return *left == *right
}

func pruneOAuthGenerations(root *os.Root, directory, keep string) error {
	values, err := readBoundedEntries(root, directory, 64)
	if err != nil {
		return err
	}
	child, err := root.OpenRoot(directory)
	if err != nil {
		return err
	}
	defer child.Close()
	for _, value := range values {
		if value.Name() != keep+".json" {
			if err = child.Remove(value.Name()); err != nil {
				return err
			}
		}
	}
	return home.SyncRootDirectory(child, ".")
}
func (f *fileAuthority) recoverOAuthGenerations() error {
	snapshot, err := f.oauthSnapshot()
	if err != nil {
		return err
	}
	for _, value := range snapshot.Applications {
		d, e := f.root.OpenRoot("adapters/oauth-applications/" + value.ApplicationID)
		if e != nil {
			return e
		}
		e = pruneOAuthGenerations(d, "credentials", value.CredentialGeneration)
		_ = d.Close()
		if e != nil {
			return e
		}
	}
	for _, value := range snapshot.Grants {
		d, e := f.root.OpenRoot("adapters/oauth-grants/" + value.GrantID)
		if e != nil {
			return e
		}
		keep := ""
		if value.TokenGeneration != nil {
			keep = *value.TokenGeneration
		}
		if _, stat := d.Lstat("tokens"); stat == nil {
			e = pruneOAuthGenerations(d, "tokens", keep)
		}
		_ = d.Close()
		if e != nil {
			return e
		}
	}
	return nil
}

func (f *fileAuthority) installOAuthOwned(parent, id, descriptor string, value any, secretDir, secretID string, secret any) error {
	stageName := ".staging-" + randomHex()
	stagePath := parent + "/" + stageName
	if err := f.root.Mkdir(stagePath, 0o700); err != nil {
		return err
	}
	ok := false
	defer func() {
		if !ok {
			_ = f.root.RemoveAll(stagePath)
		}
	}()
	stage, err := f.root.OpenRoot(stagePath)
	if err != nil {
		return err
	}
	defer stage.Close()
	raw, _ := json.Marshal(value)
	if err = writeNewFile(stage, descriptor, raw); err != nil {
		return err
	}
	if secretDir != "" {
		if err = stage.Mkdir(secretDir, 0o700); err != nil {
			return err
		}
		sr, e := stage.OpenRoot(secretDir)
		if e != nil {
			return e
		}
		secretRaw, _ := json.Marshal(secret)
		e = writeNewFile(sr, secretID+".json", secretRaw)
		if e == nil {
			e = home.SyncRootDirectory(sr, ".")
		}
		_ = sr.Close()
		if e != nil {
			return e
		}
	}
	if err = home.SyncRootDirectory(stage, "."); err != nil {
		return err
	}
	if err = stage.Close(); err != nil {
		return err
	}
	if err = f.root.Rename(stagePath, parent+"/"+id); err != nil {
		return err
	}
	ok = true
	return home.SyncRootDirectory(f.root, parent)
}
