package main

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"
)

func TestOAuthConsentExchangeRefreshAndAccountBoundary(t *testing.T) {
	f := &fixture{oauth: fixtureOAuth{secret: fixtureRandom()}}
	verifier := fixtureRandom()
	challenge := sha256.Sum256([]byte(verifier))
	query := url.Values{"client_id": {fixtureClientID}, "redirect_uri": {fixtureCallback}, "response_type": {"code"}, "scope": {fixtureScope}, "state": {"ordinary-state"}, "code_challenge_method": {"S256"}, "code_challenge": {base64.RawURLEncoding.EncodeToString(challenge[:])}}
	for _, account := range []string{"account-a", "account-b"} {
		r := httptest.NewRequest("POST", "/oauth/authorize?"+query.Encode(), strings.NewReader("account="+account))
		r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		w := httptest.NewRecorder()
		f.authorize(w, r)
		callback, err := url.Parse(w.Header().Get("Location"))
		if err != nil || w.Code != 303 || callback.Query().Get("state") != "ordinary-state" {
			t.Fatal("consent did not preserve callback state")
		}
		form := url.Values{"client_id": {fixtureClientID}, "client_secret": {f.oauth.secret}, "grant_type": {"authorization_code"}, "code": {callback.Query().Get("code")}, "redirect_uri": {fixtureCallback}, "code_verifier": {verifier}}
		exchange := func(form url.Values) (int, map[string]any) {
			t.Helper()
			r := httptest.NewRequest("POST", "/oauth/token", strings.NewReader(form.Encode()))
			r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
			w := httptest.NewRecorder()
			f.token(w, r)
			var result map[string]any
			if json.Unmarshal(w.Body.Bytes(), &result) != nil {
				t.Fatal("invalid token response")
			}
			return w.Code, result
		}
		status, tokens := exchange(form)
		if status != 200 || tokens["refresh_token"] == nil {
			t.Fatal("code exchange failed")
		}
		if status, _ := exchange(form); status != 400 {
			t.Fatal("authorization code replay succeeded")
		}
		invalidCode := fixtureRandom()
		f.oauth.codes[invalidCode] = fixtureCode{account: account, challenge: query.Get("code_challenge"), redirect: fixtureCallback, scope: fixtureScope, expires: time.Now().Add(time.Minute)}
		form.Set("code", invalidCode)
		form.Set("code_verifier", fixtureRandom())
		if status, _ := exchange(form); status != 400 {
			t.Fatal("incorrect PKCE verifier was accepted")
		}
		access := tokens["access_token"].(string)
		r = httptest.NewRequest("GET", "/gmail/v1/users/me/profile", nil)
		r.Header.Set("Authorization", "Bearer "+access)
		if f.requestAccount(r) != account {
			t.Fatal("access token changed account")
		}
		f.oauth.tokens[access] = fixtureToken{account: account, scope: fixtureScope, expires: time.Now().Add(-time.Second)}
		if f.requestAccount(r) != "anonymous" {
			t.Fatal("expired token remained valid")
		}
		form = url.Values{"client_id": {fixtureClientID}, "client_secret": {f.oauth.secret}, "grant_type": {"refresh_token"}, "refresh_token": {tokens["refresh_token"].(string)}}
		status, tokens = exchange(form)
		if status != 200 || tokens["refresh_token"] != nil {
			t.Fatal("refresh failed or replaced original refresh token")
		}
		r.Header.Set("Authorization", "Bearer "+tokens["access_token"].(string))
		if f.requestAccount(r) != account {
			t.Fatal("refresh changed account")
		}
	}
	if len(f.traces()) != 0 {
		t.Fatal("OAuth routes stored protected request values")
	}
}

func TestOAuthRejectsUnregisteredRedirectAndSupportsDenial(t *testing.T) {
	f := &fixture{oauth: fixtureOAuth{secret: fixtureRandom()}}
	challenge := sha256.Sum256([]byte(fixtureRandom()))
	query := url.Values{"client_id": {fixtureClientID}, "redirect_uri": {"https://unregistered.example/callback"}, "response_type": {"code"}, "scope": {fixtureScope}, "state": {"state"}, "code_challenge_method": {"S256"}, "code_challenge": {base64.RawURLEncoding.EncodeToString(challenge[:])}}
	w := httptest.NewRecorder()
	f.authorize(w, httptest.NewRequest("GET", "/oauth/authorize?"+query.Encode(), nil))
	if w.Code != 400 || w.Header().Get("Location") != "" {
		t.Fatal("unregistered redirect was followed")
	}
	query.Set("redirect_uri", fixtureCallback)
	r := httptest.NewRequest("POST", "/oauth/authorize?"+query.Encode(), strings.NewReader("account=deny"))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w = httptest.NewRecorder()
	f.authorize(w, r)
	location, _ := url.Parse(w.Header().Get("Location"))
	if w.Code != 303 || location.Query().Get("error") != "access_denied" || len(f.oauth.codes) != 0 {
		t.Fatal("denial issued a code or lost the error")
	}
}

func TestNotionOAuthRegistrationPKCERefreshAndAccountBoundary(t *testing.T) {
	f := &fixture{oauth: fixtureOAuth{secret: fixtureRandom(), notionRedirects: map[string]struct{}{}}}
	redirect := fixtureNotionCallback + "?attemptId=mcp_oauth:" + strings.Repeat("a", 32)
	register := httptest.NewRequest("POST", "/oauth/register", strings.NewReader(`{"redirect_uris":["`+redirect+`"]}`))
	register.Header.Set("Content-Type", "application/json")
	registerResponse := httptest.NewRecorder()
	f.notionOAuthRegister(registerResponse, register)
	if registerResponse.Code != 201 {
		t.Fatalf("dynamic registration failed: %d", registerResponse.Code)
	}

	verifier := fixtureRandom()
	challenge := sha256.Sum256([]byte(verifier))
	query := url.Values{
		"client_id": {fixtureNotionClientID}, "redirect_uri": {redirect}, "response_type": {"code"},
		"scope": {fixtureNotionScope + " offline_access"}, "state": {"notion-state"},
		"code_challenge_method": {"S256"}, "code_challenge": {base64.RawURLEncoding.EncodeToString(challenge[:])},
	}
	consent := httptest.NewRecorder()
	f.notionAuthorize(consent, httptest.NewRequest("GET", "/oauth/authorize?"+query.Encode(), nil), query)
	if consent.Code != 200 || !strings.Contains(consent.Body.String(), "Synthetic Notion consent") {
		t.Fatal("Notion consent page was not rendered")
	}
	post := httptest.NewRequest("POST", "/oauth/authorize?"+query.Encode(), strings.NewReader("account=account-b"))
	post.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	approved := httptest.NewRecorder()
	f.notionAuthorize(approved, post, query)
	callback, err := url.Parse(approved.Header().Get("Location"))
	if err != nil || approved.Code != http.StatusSeeOther || callback.Query().Get("state") != "notion-state" {
		t.Fatal("Notion consent did not return a callback")
	}

	exchange := func(form url.Values) (int, map[string]any) {
		t.Helper()
		req := httptest.NewRequest("POST", "/oauth/token", strings.NewReader(form.Encode()))
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		response := httptest.NewRecorder()
		f.token(response, req)
		var value map[string]any
		if json.Unmarshal(response.Body.Bytes(), &value) != nil {
			t.Fatal("invalid Notion token response")
		}
		return response.Code, value
	}
	form := url.Values{
		"client_id": {fixtureNotionClientID}, "client_secret": {f.oauth.secret}, "grant_type": {"authorization_code"},
		"code": {callback.Query().Get("code")}, "redirect_uri": {redirect}, "code_verifier": {verifier},
	}
	status, tokens := exchange(form)
	if status != 200 || tokens["refresh_token"] == nil || tokens["scope"] != fixtureNotionScope {
		t.Fatalf("Notion code exchange returned unexpected result: %d", status)
	}
	access := tokens["access_token"].(string)
	request := httptest.NewRequest("GET", "/notion/mcp", nil)
	request.Header.Set("Authorization", "Bearer "+access)
	if f.requestAccount(request) != "account-b" {
		t.Fatal("Notion access token changed account")
	}
	f.oauth.tokens[access] = fixtureToken{account: "account-b", scope: fixtureNotionScope, expires: time.Now().Add(-time.Second)}
	if f.requestAccount(request) != "anonymous" {
		t.Fatal("expired Notion access token remained valid")
	}
	refresh := url.Values{
		"client_id": {fixtureNotionClientID}, "client_secret": {f.oauth.secret}, "grant_type": {"refresh_token"},
		"refresh_token": {tokens["refresh_token"].(string)},
	}
	status, refreshed := exchange(refresh)
	if status != 200 || refreshed["refresh_token"] != nil {
		t.Fatal("Notion refresh failed or replaced the refresh token")
	}
	request.Header.Set("Authorization", "Bearer "+refreshed["access_token"].(string))
	if f.requestAccount(request) != "account-b" {
		t.Fatal("Notion refresh changed account")
	}
	if len(f.traces()) != 1 || f.traces()[0].Path != "/oauth/register" {
		t.Fatal("Notion OAuth trace included protected routes or unexpected values")
	}
}
