package provider

import (
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
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		attemptID := strings.TrimPrefix(r.URL.Path, openRouterCallbackPath)
		if len(r.URL.RawQuery) > 8192 {
			writeCallbackPage(w, http.StatusBadRequest, "Noema could not complete this provider connection.")
			return
		}
		codes := r.URL.Query()["code"]
		if !validAttemptID(attemptID) || len(codes) == 0 || codes[0] == "" {
			writeCallbackPage(w, http.StatusBadRequest, "Noema could not complete this provider connection.")
			return
		}
		if _, err := s.CompleteCallback(r.Context(), attemptID, codes[0]); err != nil {
			writeCallbackPage(w, http.StatusBadRequest, "Noema could not complete this provider connection.")
			return
		}
		writeCallbackPage(w, http.StatusOK, "Authentication completed.")
	})
}

func writeCallbackPage(w http.ResponseWriter, status int, message string) {
	w.WriteHeader(status)
	_, _ = w.Write([]byte("<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema Provider OAuth</title><main><p>" + message + "</p><p><a href=\"/\">Return to Noema</a></p></main>"))
}
