package web

import "net/http"

const notFoundBody = "not found"

// WriteNotFound emits the server's plain-text route miss response.
func WriteNotFound(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	w.WriteHeader(http.StatusNotFound)
	if r.Method != http.MethodHead {
		_, _ = w.Write([]byte(notFoundBody))
	}
}

// NotFoundHandler returns the server's plain-text route miss handler.
func NotFoundHandler() http.Handler {
	return http.HandlerFunc(WriteNotFound)
}
