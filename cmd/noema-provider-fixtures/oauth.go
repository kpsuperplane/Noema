package main

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"html"
	"net/http"
	"net/url"
	"strings"
	"time"
)

const fixtureClientID = "noema-synthetic-gmail"
const fixtureCallback = "https://noema.kevinpei.com/adapter/oauth/callback"
const fixtureScope = "https://www.googleapis.com/auth/gmail.readonly"
const fixtureNotionClientID = "noema-synthetic-notion"
const fixtureNotionScope = "notion.read"
const fixtureNotionCallback = "https://noema.kevinpei.com/mcp/oauth/callback"
const fixtureNotionResource = "https://noema.kevinpei.com/__pa-replay/notion/mcp"
const fixtureNotionResourceMetadata = "https://noema.kevinpei.com/__pa-replay/.well-known/oauth-protected-resource"
const fixtureNotionIssuer = "https://noema.kevinpei.com/__pa-replay"
const fixtureNotionAuthEndpoint = fixtureNotionIssuer + "/oauth/authorize"
const fixtureNotionTokenEndpoint = fixtureNotionIssuer + "/oauth/token"
const fixtureNotionRegistrationEndpoint = fixtureNotionIssuer + "/oauth/register"

type fixtureCode struct {
	account, challenge, redirect, scope string
	expires                             time.Time
}

type fixtureToken struct {
	account string
	scope   string
	expires time.Time
}

type fixtureOAuth struct {
	secret          string
	codes           map[string]fixtureCode
	tokens          map[string]fixtureToken
	refresh         map[string]string
	notionRedirects map[string]struct{}
}

func fixtureRandom() string {
	value := make([]byte, 32)
	if _, err := rand.Read(value); err != nil {
		panic(err)
	}
	return base64.RawURLEncoding.EncodeToString(value)
}

func (f *fixture) authorize(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Referrer-Policy", "no-referrer")
	if f.oauth.secret == "" {
		writeJSON(w, 503, map[string]string{"error": "temporarily_unavailable"})
		return
	}
	if r.Method != "GET" && r.Method != "POST" {
		writeJSON(w, 405, map[string]string{"error": "invalid_request"})
		return
	}
	q := r.URL.Query()
	if q.Get("client_id") == fixtureNotionClientID {
		f.notionAuthorize(w, r, q)
		return
	}
	challenge, err := base64.RawURLEncoding.DecodeString(q.Get("code_challenge"))
	if q.Get("client_id") != fixtureClientID || q.Get("redirect_uri") != fixtureCallback || q.Get("response_type") != "code" || q.Get("scope") != fixtureScope || q.Get("state") == "" || len(q.Get("state")) > 1024 || q.Get("code_challenge_method") != "S256" || err != nil || len(challenge) != 32 {
		writeJSON(w, 400, map[string]string{"error": "invalid_request"})
		return
	}
	for _, values := range q {
		if len(values) != 1 {
			writeJSON(w, 400, map[string]string{"error": "invalid_request"})
			return
		}
	}
	if r.Method == "GET" {
		writeText(w, 200, "text/html; charset=utf-8", `<!doctype html><title>Synthetic Gmail consent</title><h1>Synthetic Gmail consent</h1><p>This test grants read-only access to synthetic mail. No real Google account is used.</p><form method="post" action="?`+html.EscapeString(q.Encode())+`"><button name="account" value="account-a">Allow Alex</button> <button name="account" value="account-b">Allow Blair</button> <button name="account" value="deny">Deny</button></form>`)
		return
	}
	r.Body = http.MaxBytesReader(w, r.Body, 4096)
	if r.ParseForm() != nil || len(r.PostForm["account"]) != 1 {
		writeJSON(w, 400, map[string]string{"error": "invalid_request"})
		return
	}
	account := r.PostForm.Get("account")
	result := url.Values{"state": {q.Get("state")}}
	if account == "deny" {
		result.Set("error", "access_denied")
	} else {
		if account != "account-a" && account != "account-b" {
			writeJSON(w, 400, map[string]string{"error": "invalid_request"})
			return
		}
		code := fixtureRandom()
		f.mu.Lock()
		if f.oauth.codes == nil {
			f.oauth.codes = map[string]fixtureCode{}
		}
		f.oauth.codes[code] = fixtureCode{account: account, challenge: q.Get("code_challenge"), redirect: fixtureCallback, scope: fixtureScope, expires: time.Now().Add(5 * time.Minute)}
		f.mu.Unlock()
		result.Set("code", code)
	}
	// Do not trace authorization URLs: they contain protected transient values.
	http.Redirect(w, r, fixtureCallback+"?"+result.Encode(), http.StatusSeeOther)
}

func (f *fixture) notionAuthorize(w http.ResponseWriter, r *http.Request, q url.Values) {
	if f.oauth.secret == "" {
		writeJSON(w, http.StatusServiceUnavailable, map[string]string{"error": "temporarily_unavailable"})
		return
	}
	challenge, err := base64.RawURLEncoding.DecodeString(q.Get("code_challenge"))
	if r.Method != http.MethodGet && r.Method != http.MethodPost || q.Get("client_id") != fixtureNotionClientID || !validNotionRedirect(q.Get("redirect_uri")) || q.Get("response_type") != "code" || !notionScopeValid(q.Get("scope")) || q.Get("state") == "" || len(q.Get("state")) > 1024 || q.Get("code_challenge_method") != "S256" || err != nil || len(challenge) != 32 {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_request"})
		return
	}
	for _, values := range q {
		if len(values) != 1 {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_request"})
			return
		}
	}
	f.mu.Lock()
	registered := false
	if f.oauth.notionRedirects != nil {
		_, registered = f.oauth.notionRedirects[q.Get("redirect_uri")]
		if !registered {
			registeredKey := notionRedirectKey(q.Get("redirect_uri"))
			for redirect := range f.oauth.notionRedirects {
				if notionRedirectKey(redirect) == registeredKey {
					registered = true
					break
				}
			}
		}
	}
	f.mu.Unlock()
	// The synthetic client is a pre-registered client as well as the client
	// returned by dynamic registration. Its registration survives fixture
	// process restarts, so a later reauthentication attempt can use a fresh
	// attemptId on the same callback path.
	if !registered && q.Get("client_id") == fixtureNotionClientID {
		registered = notionRedirectKey(q.Get("redirect_uri")) == fixtureNotionCallback
	}
	if !registered {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_redirect_uri"})
		return
	}
	if r.Method == http.MethodGet {
		writeText(w, http.StatusOK, "text/html; charset=utf-8", `<!doctype html><title>Synthetic Notion consent</title><h1>Synthetic Notion consent</h1><p>This test grants read-only access to a synthetic workspace. No real account is used.</p><form method="post" action="?`+html.EscapeString(q.Encode())+`"><button name="account" value="account-a">Allow Alex</button> <button name="account" value="account-b">Allow Blair</button> <button name="account" value="deny">Deny</button></form>`)
		return
	}
	r.Body = http.MaxBytesReader(w, r.Body, 4096)
	if r.ParseForm() != nil || len(r.PostForm["account"]) != 1 {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_request"})
		return
	}
	account := r.PostForm.Get("account")
	result := url.Values{"state": {q.Get("state")}}
	if account == "deny" {
		result.Set("error", "access_denied")
	} else {
		if account != "account-a" && account != "account-b" {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_request"})
			return
		}
		code := fixtureRandom()
		f.mu.Lock()
		if f.oauth.codes == nil {
			f.oauth.codes = map[string]fixtureCode{}
		}
		f.oauth.codes[code] = fixtureCode{account: account, challenge: q.Get("code_challenge"), redirect: q.Get("redirect_uri"), scope: q.Get("scope"), expires: time.Now().Add(5 * time.Minute)}
		f.mu.Unlock()
		result.Set("code", code)
	}
	callback, _ := url.Parse(q.Get("redirect_uri"))
	callbackQuery := callback.Query()
	for key, values := range result {
		callbackQuery.Del(key)
		for _, value := range values {
			callbackQuery.Add(key, value)
		}
	}
	callback.RawQuery = callbackQuery.Encode()
	http.Redirect(w, r, callback.String(), http.StatusSeeOther)
}

func validNotionRedirect(raw string) bool {
	value, err := url.Parse(raw)
	if err != nil || value.Scheme != "https" || value.Host != "noema.kevinpei.com" || value.Path != "/mcp/oauth/callback" || value.Fragment != "" {
		return false
	}
	query := value.Query()
	attempt := query.Get("attemptId")
	return len(query) == 1 && strings.HasPrefix(attempt, "mcp_oauth:") && len(attempt) > len("mcp_oauth:")
}

func notionRedirectKey(raw string) string {
	value, err := url.Parse(raw)
	if err != nil || !validNotionRedirect(raw) {
		return ""
	}
	value.RawQuery = ""
	return value.String()
}

func notionScopeValid(raw string) bool {
	values := strings.Fields(raw)
	if len(values) < 1 || len(values) > 2 || values[0] != fixtureNotionScope {
		return false
	}
	return len(values) == 1 || values[1] == "offline_access"
}

func (f *fixture) token(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cache-Control", "no-store")
	if r.Method != "POST" {
		writeJSON(w, 405, map[string]string{"error": "invalid_request"})
		return
	}
	r.Body = http.MaxBytesReader(w, r.Body, 16384)
	if r.ParseForm() != nil {
		writeJSON(w, 400, map[string]string{"error": "invalid_request"})
		return
	}
	q := r.PostForm
	for _, values := range q {
		if len(values) != 1 {
			writeJSON(w, 400, map[string]string{"error": "invalid_request"})
			return
		}
	}
	clientID := q.Get("client_id")
	scope := fixtureScope
	if clientID != fixtureClientID && clientID != fixtureNotionClientID || f.oauth.secret == "" || q.Get("client_secret") != f.oauth.secret {
		writeJSON(w, 401, map[string]string{"error": "invalid_client"})
		return
	}
	if clientID == fixtureNotionClientID {
		scope = fixtureNotionScope
	}
	f.mu.Lock()
	defer f.mu.Unlock()
	account := ""
	refresh := ""
	switch q.Get("grant_type") {
	case "authorization_code":
		code, ok := f.oauth.codes[q.Get("code")]
		delete(f.oauth.codes, q.Get("code"))
		challenge := sha256.Sum256([]byte(q.Get("code_verifier")))
		if ok && time.Now().Before(code.expires) && code.scope != "" && strings.HasPrefix(code.scope, scope) && q.Get("redirect_uri") == code.redirect && len(q.Get("code_verifier")) >= 43 && len(q.Get("code_verifier")) <= 128 && base64.RawURLEncoding.EncodeToString(challenge[:]) == code.challenge {
			account = code.account
			refresh = fixtureRandom()
		}
	case "refresh_token":
		account = f.oauth.refresh[q.Get("refresh_token")]
	default:
		writeJSON(w, 400, map[string]string{"error": "unsupported_grant_type"})
		return
	}
	if account == "" {
		writeJSON(w, 400, map[string]string{"error": "invalid_grant"})
		return
	}
	if f.oauth.tokens == nil {
		f.oauth.tokens = map[string]fixtureToken{}
		f.oauth.refresh = map[string]string{}
	}
	access := fixtureRandom()
	f.oauth.tokens[access] = fixtureToken{account: account, scope: scope, expires: time.Now().Add(2 * time.Minute)}
	response := map[string]any{"access_token": access, "token_type": "Bearer", "expires_in": 120, "scope": scope}
	if refresh != "" {
		f.oauth.refresh[refresh] = account
		response["refresh_token"] = refresh
	}
	// Refresh omits refresh_token, as providers may retain the original token.
	writeJSON(w, 200, response)
}
