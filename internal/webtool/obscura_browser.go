package webtool

import (
	"context"
	"errors"
	"time"
)

func executeObscuraBrowser(ctx context.Context, session *browserSession, name string, arguments map[string]any) (*browseProviderResponse, *browserProviderFailure) {
	p := session.process
	if name == BrowseInteractName && arguments["action"] == "upload_file" {
		return nil, &browserProviderFailure{
			code:    "unavailable",
			message: "Obscura browser does not support file upload",
			diagnostic: &browserDiagnostic{
				Provider: "obscura",
				Stage:    name,
				Detail:   "file upload is unavailable in the Obscura worker",
			},
		}
	}
	command, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	uncertain := name == BrowseInteractName || name == BrowseHistoryName || name == BrowseOpenName && session.publicRevision != 0
	failed := func(err error) (*browseProviderResponse, *browserProviderFailure) {
		var commandError *obscuraCommandError
		knownFailure := errors.As(err, &commandError)
		detail := "CDP response was unavailable"
		if knownFailure {
			detail = commandError.Error()
		}
		return nil, &browserProviderFailure{code: browserFailureCodeForMutation(uncertain), message: "Obscura browser operation failed", uncertain: uncertain, drop: uncertain || !knownFailure,
			diagnostic: &browserDiagnostic{Provider: "obscura", Stage: name, Detail: detail}}
	}
	// Drain previous navigation events before tracking the next action.
	if err := p.evaluate(command, "true", nil); err != nil {
		return failed(err)
	}
	p.mainDocumentStatus = 0
	switch name {
	case BrowseOpenName:
		if err := p.call(command, "Page.navigate", map[string]any{"url": arguments["url"], "waitUntil": arguments["wait_until"]}, nil); err != nil {
			return failed(err)
		}
	case BrowseInteractName:
		var found bool
		var target browseElement
		for _, element := range session.elements {
			if element.Reference == arguments["ref"] {
				target = element
				break
			}
		}
		if err := p.evaluate(command, obscuraInteractionScript(arguments, target, session.url), &found); err != nil {
			return failed(err)
		}
		if !found {
			return nil, &browserProviderFailure{code: "stale_snapshot", message: "browser element changed or is unavailable; take a new snapshot"}
		}
	case BrowseHistoryName:
		action := arguments["action"]
		method, params := "Page.reload", map[string]any{"waitUntil": "load"}
		if action != "reload" {
			var history struct {
				CurrentIndex int `json:"currentIndex"`
				Entries      []struct {
					ID int `json:"id"`
				} `json:"entries"`
			}
			if err := p.call(command, "Page.getNavigationHistory", nil, &history); err != nil {
				return failed(err)
			}
			index := history.CurrentIndex - 1
			if action == "forward" {
				index = history.CurrentIndex + 1
			}
			if index < 0 || index >= len(history.Entries) {
				return nil, &browserProviderFailure{code: "history_unavailable", message: "browser history is unavailable"}
			}
			method, params = "Page.navigateToHistoryEntry", map[string]any{"entryId": history.Entries[index].ID}
		}
		if err := p.call(command, method, params, nil); err != nil {
			return failed(err)
		}
	case BrowseWaitName:
		condition := arguments["condition"].(map[string]any)
		check := "Array.from(document.querySelectorAll('[data-noema-ref]')).some(e=>e.dataset.noemaRef===" + jsValue(condition["ref"]) + ")"
		if text, ok := condition["text"]; ok {
			check = "Boolean(document.body&&document.body.innerText.includes(" + jsValue(text) + "))"
		}
		script := `new Promise(resolve=>{const check=()=>` + check + `;if(check()){resolve(true);return;}let timer;const finish=value=>{observer.disconnect();clearTimeout(timer);resolve(value);};const observer=new MutationObserver(()=>{if(check())finish(true);});observer.observe(document,{subtree:true,childList:true,characterData:true,attributes:true});timer=setTimeout(()=>finish(false),` + jsValue(arguments["timeout_ms"]) + `);})`
		var matched bool
		if err := p.evaluate(command, script, &matched); err != nil {
			return failed(err)
		}
		if !matched {
			return nil, &browserProviderFailure{code: "timeout", message: "browser wait timed out"}
		}
	}
	// Upstream settles pending navigation after evaluation, including event handlers.
	if err := p.evaluate(command, "new Promise(resolve=>setTimeout(()=>resolve(true),250))", nil); err != nil {
		return failed(err)
	}
	var raw browserRawSnapshot
	if err := p.evaluate(command, "("+browserSnapshotScript+")()", &raw); err != nil {
		return failed(err)
	}
	var screenshot struct {
		Data string `json:"data"`
	}
	if err := p.call(command, "Page.captureScreenshot", map[string]any{"format": "png"}, &screenshot); err != nil {
		var commandError *obscuraCommandError
		if !errors.As(err, &commandError) {
			return failed(err)
		}
	} else {
		raw.Screenshot = screenshot.Data
	}
	maxChars := 12000
	if value, ok := integer(arguments["max_chars"]); ok {
		maxChars = int(value)
	}
	response, err := browserSnapshotResponse(&raw, maxChars, "obscura")
	if err != nil {
		return failed(err)
	}
	if name == BrowseInteractName && p.mainDocumentStatus >= 500 {
		response.State = "outcome_uncertain"
	}
	return response, nil
}
