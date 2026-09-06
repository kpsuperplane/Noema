package provider

import (
	"github.com/kpsuperplane/noema/internal/publicpage"
	"net/http"
	"strings"
)

const openRouterCallbackPath = "/provider/oauth/callback/"

// CallbackHandler completes OpenRouter OAuth callbacks without exposing provider values.
func (s *OpenRouterService) CallbackHandler() http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		if r.Method != http.MethodGet {
			w.Header().Set("Allow", http.MethodGet)
			publicpage.Callback(w, http.StatusMethodNotAllowed, false)
			return
		}
		attemptID := strings.TrimPrefix(r.URL.Path, openRouterCallbackPath)
		if len(r.URL.RawQuery) > 8192 {
			publicpage.Callback(w, http.StatusBadRequest, false)
			return
		}
		codes := r.URL.Query()["code"]
		if !validAttemptID(attemptID) || len(codes) == 0 || codes[0] == "" {
			publicpage.Callback(w, http.StatusBadRequest, false)
			return
		}
		if _, err := s.CompleteCallback(r.Context(), attemptID, codes[0]); err != nil {
			publicpage.Callback(w, http.StatusBadRequest, false)
			return
		}
		publicpage.Callback(w, http.StatusOK, true)
	})
}
