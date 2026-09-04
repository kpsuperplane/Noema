package auth

import (
	"context"
	stderrors "errors"
	"io"
	"mime"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/go-oauth2/oauth2/v4"
	oautherrors "github.com/go-oauth2/oauth2/v4/errors"
	"github.com/go-oauth2/oauth2/v4/models"
	oauthserver "github.com/go-oauth2/oauth2/v4/server"
)

const (
	oauthGrantAuthorizationCode = "authorization_code"
	oauthGrantRefreshToken      = "refresh_token"
)

var (
	errInvalidOAuthRequest = oautherrors.ErrInvalidRequest
	errInvalidOAuthGrant   = oautherrors.ErrInvalidGrant
	errUnsupportedGrant    = oautherrors.ErrUnsupportedGrantType
)

// nativeOAuthProtocol keeps OAuth protocol rules in the maintained server library.
type nativeOAuthProtocol struct {
	server *oauthserver.Server
}

type nativeTokenRequest struct {
	clientID  string
	redirect  string
	code      string
	verifier  string
	refresh   string
	requestID string
}

func newNativeOAuthProtocol() nativeOAuthProtocol {
	server := oauthserver.NewServer(&oauthserver.Config{
		TokenType:                   "bearer",
		AllowedResponseTypes:        []oauth2.ResponseType{oauth2.Code},
		AllowedGrantTypes:           []oauth2.GrantType{oauth2.AuthorizationCode, oauth2.Refreshing},
		AllowedCodeChallengeMethods: []oauth2.CodeChallengeMethod{oauth2.CodeChallengeS256},
		ForcePKCE:                   true,
	}, nil)
	server.SetClientInfoHandler(func(r *http.Request) (string, string, error) {
		return r.FormValue("client_id"), "", nil
	})
	return nativeOAuthProtocol{server: server}
}

func (p nativeOAuthProtocol) authorization(raw string) (authorizationRequest, error) {
	if raw == "" || len(raw) > nativeOAuthLimit {
		return authorizationRequest{}, errInvalidOAuthRequest
	}
	if _, err := uniqueParameters(raw); err != nil {
		return authorizationRequest{}, errInvalidOAuthRequest
	}
	request := &http.Request{Method: http.MethodGet, URL: &url.URL{RawQuery: raw}}
	validated, err := p.server.ValidationAuthorizeRequest(request)
	if err != nil || !validClientAndRedirect(validated.ClientID, validated.RedirectURI) ||
		len(validated.State) < 32 || len(validated.State) > 256 ||
		!validURLText(validated.CodeChallenge, 43, 43) ||
		validated.Scope != "" && validated.Scope != "noema" {
		return authorizationRequest{}, errInvalidOAuthRequest
	}
	return authorizationRequest{
		clientID:  validated.ClientID,
		redirect:  validated.RedirectURI,
		state:     validated.State,
		challenge: validated.CodeChallenge,
		ios:       strings.HasPrefix(validated.ClientID, "noema-ios:"),
	}, nil
}

func (p nativeOAuthProtocol) token(
	r *http.Request,
	parameters map[string]string,
) (string, nativeTokenRequest, error) {
	grant := oauth2.GrantType(parameters["grant_type"])
	if grant.String() == "" || !p.server.CheckGrantType(grant) {
		return "", nativeTokenRequest{}, errUnsupportedGrant
	}
	form := make(url.Values, len(parameters))
	for key, value := range parameters {
		form.Set(key, value)
	}
	request := r.Clone(r.Context())
	request.Form = form
	request.PostForm = form
	validatedGrant, validated, err := p.server.ValidationTokenRequest(request)
	if err != nil {
		return "", nativeTokenRequest{}, errInvalidOAuthRequest
	}
	return validatedGrant.String(), nativeTokenRequest{
		clientID:  validated.ClientID,
		redirect:  validated.RedirectURI,
		code:      validated.Code,
		verifier:  validated.CodeVerifier,
		refresh:   validated.Refresh,
		requestID: parameters["refresh_request_id"],
	}, nil
}

func (p nativeOAuthProtocol) redirect(
	w http.ResponseWriter,
	request authorizationRequest,
	code string,
	failure string,
) {
	data := make(map[string]interface{}, 1)
	if failure != "" {
		data["error"] = failure
	} else {
		data["code"] = code
	}
	destination, err := p.server.GetRedirectURI(&oauthserver.AuthorizeRequest{
		ResponseType: oauth2.Code,
		RedirectURI:  request.redirect,
		State:        request.state,
	}, data)
	if err != nil {
		writeAuthorizationError(w, http.StatusInternalServerError, "service")
		return
	}
	w.Header().Set("Location", destination)
	w.WriteHeader(http.StatusFound)
}

func (p nativeOAuthProtocol) writeToken(w http.ResponseWriter, response nativeTokenResponse) {
	token := models.NewToken()
	token.SetAccess(response.AccessToken)
	token.SetRefresh(response.RefreshToken)
	token.SetAccessExpiresIn(time.Duration(response.ExpiresIn) * time.Second)
	token.SetScope(response.Scope)
	writeJSON(w, http.StatusOK, p.server.GetTokenData(token))
}

func (p nativeOAuthProtocol) writeError(w http.ResponseWriter, failure error) {
	data, _, _ := p.server.GetErrorData(context.Background(), failure)
	code, _ := data["error"].(string)
	writeJSON(w, http.StatusBadRequest, map[string]string{"error": code})
}

func parseAuthorizationRequest(raw string) (authorizationRequest, error) {
	return newNativeOAuthProtocol().authorization(raw)
}

func uniqueParameters(raw string) (map[string]string, error) {
	values, err := url.ParseQuery(raw)
	if err != nil {
		return nil, err
	}
	result := make(map[string]string, len(values))
	for key, entries := range values {
		if len(entries) != 1 {
			return nil, stderrors.New("duplicate OAuth parameter")
		}
		result[key] = entries[0]
	}
	return result, nil
}

func readUniqueForm(w http.ResponseWriter, r *http.Request, limit int64) (map[string]string, error) {
	r.Body = http.MaxBytesReader(w, r.Body, limit)
	data, err := io.ReadAll(r.Body)
	if err != nil {
		return nil, err
	}
	return uniqueParameters(string(data))
}

func formContentType(raw string) bool {
	kind, _, err := mime.ParseMediaType(raw)
	return err == nil && kind == "application/x-www-form-urlencoded"
}
