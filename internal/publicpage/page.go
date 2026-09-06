// Package publicpage renders supporting pages without an authenticated app session.
package publicpage

import (
	"embed"
	"html/template"
	"net/http"
)

//go:embed page.html buttons.html
var templates embed.FS
var page = template.Must(template.ParseFS(templates, "page.html", "buttons.html"))

type Page struct {
	Return             bool
	Title, Intro, Note string
	ClientName, CSRF   string
}

func Write(w http.ResponseWriter, status int, value Page) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Referrer-Policy", "no-referrer")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.WriteHeader(status)
	_ = page.ExecuteTemplate(w, "page.html", value)
}

func Callback(w http.ResponseWriter, status int, complete bool) {
	value := Page{Return: true, Title: "This connection needs attention", Intro: "Return to Noema to try again.", Note: "Return to the chat or Settings page where you started this connection."}
	if complete {
		value.Title = "You’re signed in"
		value.Intro = "Return to Noema to finish setup."
		value.Note = "Your account is connected. Return to the chat or Settings page where you started to review access and finish setup."
	}
	Write(w, status, value)
}
