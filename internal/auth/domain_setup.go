package auth

import (
	"net/http"
	"net/url"
	"strings"
	"sync"

	"github.com/kpsuperplane/noema/internal/web"
)

// DomainSetup serves only first-run address confirmation and static assets.
// The caller starts normal services with the saved configuration.
func DomainSetup(config Config, recovery *Recovery, ready <-chan struct{}) (http.Handler, <-chan Config) {
	configured := make(chan Config, 1)
	var mu sync.Mutex
	complete := false
	assets := web.NewAssetHandler()
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		setResponsePolicy(w.Header())
		if r.Method == http.MethodGet && r.URL.Path == "/auth/status" {
			writeJSON(w, http.StatusOK, map[string]string{"state": "domain_setup_required", "mode": "required"})
			return
		}
		if r.Method == http.MethodGet && (web.IsSPAPath(r.URL.Path) || strings.HasPrefix(r.URL.Path, "/assets/") || r.URL.Path == "/favicon.ico") {
			assets.ServeHTTP(w, r)
			return
		}
		if r.Method != http.MethodPost || r.URL.Path != "/auth/domain" {
			writeAuthError(w, http.StatusForbidden, "domain_setup_required")
			return
		}
		var input struct {
			Origin string `json:"origin"`
		}
		if decodeJSON(w, r, recoveryBodyLimit, &input) != nil {
			writeAuthError(w, http.StatusBadRequest, "invalid_origin")
			return
		}
		origin, err := url.Parse(input.Origin)
		if err != nil {
			writeAuthError(w, http.StatusBadRequest, "invalid_origin")
			return
		}
		next, err := canonicalConfig(input.Origin, origin.Hostname(), false)
		if err != nil || next.Origin != input.Origin || next.Authority != r.Host || r.Header.Get("Origin") != next.Origin {
			writeAuthError(w, http.StatusBadRequest, "invalid_origin")
			return
		}
		mu.Lock()
		defer mu.Unlock()
		if complete {
			writeAuthError(w, http.StatusConflict, "domain_already_configured")
			return
		}
		recovery.mu.Lock()
		defer recovery.mu.Unlock()
		document, err := readConfigDocument(recovery.path)
		if err != nil {
			writeAuthError(w, http.StatusInternalServerError, "configuration_unavailable")
			return
		}
		settings, err := childMap(document, "web")
		if err != nil {
			writeAuthError(w, http.StatusInternalServerError, "configuration_unavailable")
			return
		}
		if settings["public_origin"] != nil {
			writeAuthError(w, http.StatusConflict, "domain_already_configured")
			return
		}
		settings["public_origin"], settings["rp_id"] = next.Origin, next.RPID
		if err := writeConfigDocument(recovery.path, document); err != nil {
			writeAuthError(w, http.StatusInternalServerError, "configuration_unavailable")
			return
		}
		config.Authority, config.Origin, config.RPID, config.Secure = next.Authority, next.Origin, next.RPID, next.Secure
		config.DomainSetupRequired = false
		complete = true
		configured <- config
		select {
		case <-ready:
			w.WriteHeader(http.StatusNoContent)
		case <-r.Context().Done():
		}
	}), configured
}
