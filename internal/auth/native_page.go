package auth

import (
	"github.com/kpsuperplane/noema/internal/publicpage"
	"net/http"
)

func writeConsentPage(w http.ResponseWriter, request authorizationRequest, csrf string) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if request.ios {
		w.Header().Set("Content-Security-Policy", "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self' noema:")
	}
	publicpage.Write(w, http.StatusOK, publicpage.Page{
		Title:      "Connect " + displayName(request.clientID) + "?",
		Intro:      "Give this app access to your Noema.",
		ClientName: displayName(request.clientID), CSRF: csrf,
	})
}

func writeAuthorizationError(w http.ResponseWriter, status int, failure string) {
	value := publicpage.Page{Title: "This connection request is invalid", Intro: "Start a new connection from the app.", Note: "Return to the Noema app and try again. This request granted no new access."}
	switch failure {
	case "expired":
		value.Title = "This connection request expired"
	case "unverified":
		value.Title = "This connection could not be verified"
	case "service":
		value.Title = "Noema could not finish connecting"
		value.Intro = "Return to the app and try again."
		value.Note = "The server could not complete this request."
	}
	publicpage.Write(w, status, value)
}
