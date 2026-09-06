package main

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
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
		f.oauth.codes[invalidCode] = fixtureCode{account, query.Get("code_challenge"), fixtureCallback, time.Now().Add(time.Minute)}
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
		f.oauth.tokens[access] = fixtureToken{account, time.Now().Add(-time.Second)}
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
