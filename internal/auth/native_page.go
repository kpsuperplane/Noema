package auth

import (
	"html/template"
	"net/http"
)

const nativePageStyle = `
:root{color-scheme:light;font-family:system-ui,sans-serif;background:#fcfaf5;color:#17160f}
*{box-sizing:border-box}body{margin:0}main{display:grid;min-height:100svh;place-items:center;padding:1.5rem}
section{display:flex;width:min(100%,30rem);flex-direction:column;gap:1rem;padding:1.5rem;border:1px solid #ddd6c8;border-radius:.75rem;background:#fff}
h1,p{margin:0}h1{font-size:clamp(1.75rem,6vw,2rem);overflow-wrap:anywhere}.muted{color:#5e5a4b}
.detail{padding:.75rem;border-radius:.625rem;background:#f7f2e8}form{display:grid;grid-template-columns:1fr 1fr;gap:.5rem}
button{min-height:2.75rem;padding:.5rem 1rem;border:1px solid #176046;border-radius:.625rem;font:inherit;font-weight:650}
button:focus-visible{outline:2px solid;outline-offset:3px}.approve{background:#176046;color:#fcfaf5}.deny{background:#fff;color:#17160f;border-color:#8c897f}
@media(max-width:40rem){main{place-items:start stretch;padding:0}section{width:100%;min-height:100svh;border:0;border-radius:0}}
@media(forced-colors:active){section,button{border-color:CanvasText}}`

var consentPage = template.Must(template.New("consent").Parse(`<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#fcfaf5"><title>Connect {{.Name}} · Noema</title><style>` + nativePageStyle + `</style></head>
<body><main><section aria-labelledby="consent-title" aria-describedby="consent-summary consent-access">
<p class="muted">Client connection</p><h1 id="consent-title">Connect <span translate="no">{{.Name}}</span>?</h1>
<p class="muted" id="consent-summary">Connect only if you started this request. Denying it grants no new access.</p>
<div class="detail" id="consent-access"><p><strong>Complete Noema access</strong></p>
<p class="muted"><span translate="no">{{.Name}}</span> can access all data and actions available through Noema.</p>
<p class="muted">You can revoke this client later in Settings.</p></div>
<form method="post" action="/oauth/authorize"><input type="hidden" name="csrf" value="{{.CSRF}}">
<button class="deny" type="submit" name="decision" value="deny">Deny</button>
<button class="approve" type="submit" name="decision" value="approve">Connect client</button></form>
</section></main></body></html>`))

var authorizationErrorPage = template.Must(template.New("error").Parse(`<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#fcfaf5"><title>{{.Title}} · Noema</title><style>` + nativePageStyle + `</style></head>
<body><main><section aria-labelledby="error-title"><p class="muted">Authorization error</p>
<h1 id="error-title">{{.Title}}</h1><p class="muted">{{.Summary}}</p>
<div class="detail"><p><strong>What to do next</strong></p><p class="muted">{{.Next}}</p></div>
<p class="muted">This attempt granted no client access.</p></section></main></body></html>`))

func writeConsentPage(w http.ResponseWriter, request authorizationRequest, csrf string) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if request.ios {
		w.Header().Set("Content-Security-Policy", "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self' noema:")
	}
	w.WriteHeader(http.StatusOK)
	_ = consentPage.Execute(w, struct{ Name, CSRF string }{displayName(request.clientID), csrf})
}

func writeAuthorizationError(w http.ResponseWriter, status int, failure string) {
	value := struct{ Title, Summary, Next string }{
		"Authorization request not accepted",
		"The connection request is invalid or incomplete.",
		"Close this window. Then start the connection again in the Noema app.",
	}
	switch failure {
	case "expired":
		value.Title = "Authorization expired"
		value.Summary = "This connection request is no longer available."
	case "unverified":
		value.Title = "Authorization could not be verified"
		value.Summary = "Noema did not accept this authorization response."
	case "service":
		value.Title = "Noema could not finish authorization"
		value.Summary = "The server could not complete this request."
		value.Next = "Close this window. Then return to Noema and try again."
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(status)
	_ = authorizationErrorPage.Execute(w, value)
}
