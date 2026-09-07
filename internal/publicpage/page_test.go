package publicpage

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestConsentEscapesValuesAndKeepsNativeFormContract(t *testing.T) {
	response := httptest.NewRecorder()
	Write(response, http.StatusOK, Page{Title: "Connect app", ClientName: `<script>alert(1)</script>`, CSRF: `"><input name="decision" value="approve">`})
	body := response.Body.String()
	for _, unsafe := range []string{"<script>", `<input name="decision"`} {
		if strings.Contains(body, unsafe) {
			t.Fatalf("unescaped HTML value")
		}
	}
	for _, required := range []string{`method="post"`, `action="/oauth/authorize"`, `name="csrf"`, `value="deny" name="decision"`, `value="approve" name="decision"`, "all data and actions", "&lt;script&gt;"} {
		if !strings.Contains(body, required) {
			t.Errorf("missing consent contract: %s", required)
		}
	}
	if response.Header().Get("Cache-Control") != "no-store" || response.Header().Get("Referrer-Policy") != "no-referrer" {
		t.Fatal("public authorization page must not cache or forward callback URLs")
	}
}
