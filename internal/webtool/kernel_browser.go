package webtool

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	kernelResponseLimit   = 2 << 20
	kernelScreenshotLimit = 900_000
	kernelUploadLimit     = 256 << 10
)

type browserUploadFile struct {
	Filename, MediaType string
	Bytes               []byte
}

type kernelCreateResponse struct {
	SessionID string `json:"session_id"`
}

type kernelExecuteResponse struct {
	Success bool            `json:"success"`
	Result  json.RawMessage `json:"result"`
}

type kernelCommandResult struct {
	OK                 bool               `json:"ok"`
	Snapshot           *kernelRawSnapshot `json:"snapshot"`
	ElementFound       *bool              `json:"element_found"`
	HistoryAvailable   *bool              `json:"history_available"`
	MainDocumentStatus *int               `json:"main_document_status"`
}

type kernelRawSnapshot struct {
	URL, Title, Text string
	Elements         []browseElement
	Screenshot       string `json:"screenshot"`
	Width, Height    int
}

type kernelCallFailure struct {
	code, stage, detail string
	uncertain           bool
	auth                bool
}

func (s *Service) executeKernelBrowser(ctx context.Context, session *browserSession, name string, arguments map[string]any, upload *browserUploadFile) (*browseProviderResponse, *browserProviderFailure) {
	account, err := s.accounts.LoadAccount(ctx, session.providerAccountID)
	if err != nil || account.Metadata.CredentialRevision() != session.credentialRevision || !s.browserProviderAvailable(account) {
		return nil, &browserProviderFailure{code: "unavailable", message: "Kernel browser account is unavailable", drop: true}
	}
	if session.kernelID == "" {
		if name != BrowseOpenName {
			return nil, &browserProviderFailure{code: "session_not_found", message: "Kernel browser session is unavailable", drop: true}
		}
		var created kernelCreateResponse
		failure := s.kernelJSON(ctx, account.ID, session.credentialRevision, http.MethodPost, "/browsers",
			map[string]any{"headless": true, "stealth": true, "timeout_seconds": 1800}, &created, false, "create")
		if failure != nil {
			return nil, kernelBrowserFailure(failure, false)
		}
		if !safeKernelSessionID(created.SessionID) {
			return nil, &browserProviderFailure{code: "unavailable", message: "Kernel returned an invalid browser session", drop: true}
		}
		session.kernelID = created.SessionID
	}
	maxChars := 12000
	if value, ok := integer(arguments["max_chars"]); ok {
		maxChars = int(value)
	}
	script, err := kernelScript(name, arguments, upload)
	if err != nil {
		return nil, &browserProviderFailure{code: "invalid_input", message: err.Error()}
	}
	mutation := name == BrowseInteractName || name == BrowseHistoryName
	var execution kernelExecuteResponse
	failure := s.kernelJSON(ctx, account.ID, session.credentialRevision, http.MethodPost,
		"/browsers/"+session.kernelID+"/playwright/execute", map[string]any{"code": script, "timeout_sec": 30},
		&execution, mutation, "playwright_execute")
	if failure != nil {
		return nil, kernelBrowserFailure(failure, failure.uncertain || failure.auth)
	}
	if !execution.Success || len(execution.Result) == 0 || string(execution.Result) == "null" {
		return nil, &browserProviderFailure{code: browserFailureCode(name), message: "Kernel browser operation failed",
			uncertain: mutation, drop: mutation, diagnostic: &browserDiagnostic{Provider: kernelProvider, Stage: "playwright_execute", Detail: "execution failed"}}
	}
	var result kernelCommandResult
	if json.Unmarshal(execution.Result, &result) != nil {
		return nil, &browserProviderFailure{code: browserFailureCode(name), message: "Kernel returned an invalid browser result", uncertain: mutation, drop: true}
	}
	if !result.OK {
		if result.ElementFound != nil && !*result.ElementFound {
			return nil, &browserProviderFailure{code: "element_not_found", message: "browser element is unavailable"}
		}
		if result.HistoryAvailable != nil && !*result.HistoryAvailable {
			return nil, &browserProviderFailure{code: "history_unavailable", message: "browser history is unavailable"}
		}
		return nil, &browserProviderFailure{code: browserFailureCode(name), message: "Kernel browser operation failed", uncertain: mutation, drop: mutation}
	}
	if result.Snapshot == nil {
		return nil, &browserProviderFailure{code: browserFailureCode(name), message: "Kernel browser snapshot is unavailable", uncertain: mutation, drop: mutation}
	}
	response, err := kernelBrowserResponse(result.Snapshot, maxChars)
	if err != nil {
		return nil, &browserProviderFailure{code: "unavailable", message: err.Error(), drop: true}
	}
	if name == BrowseInteractName && result.MainDocumentStatus != nil && *result.MainDocumentStatus >= 500 {
		response.State = "outcome_uncertain"
	}
	return response, nil
}

func kernelBrowserFailure(failure *kernelCallFailure, drop bool) *browserProviderFailure {
	return &browserProviderFailure{code: failure.code, message: "Kernel browser request failed", uncertain: failure.uncertain,
		drop: drop || failure.uncertain, diagnostic: &browserDiagnostic{Provider: kernelProvider, Stage: failure.stage, Detail: failure.detail}}
}

func browserFailureCode(name string) string {
	if name == BrowseInteractName || name == BrowseHistoryName {
		return "outcome_uncertain"
	}
	if name == BrowseOpenName {
		return "navigation_failed"
	}
	if name == BrowseWaitName {
		return "timeout"
	}
	return "unavailable"
}

func (s *Service) kernelJSON(ctx context.Context, accountID string, revision uint64, method, path string, body, target any, mutation bool, stage string) *kernelCallFailure {
	var data []byte
	if body != nil {
		data, _ = json.Marshal(body)
	}
	command, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	request, err := http.NewRequestWithContext(command, method, strings.TrimRight(s.endpoints[kernelProvider], "/")+path, bytes.NewReader(data))
	if err != nil {
		return &kernelCallFailure{code: "unavailable", stage: stage, detail: "request setup failed"}
	}
	request.Header.Set("Content-Type", "application/json")
	secret, err := s.accounts.LoadSecretAtRevision(command, accountID, revision)
	if err != nil || secret.Use(func(value string) error { request.Header.Set("Authorization", "Bearer "+value); return nil }) != nil {
		return &kernelCallFailure{code: "unauthenticated", stage: stage, detail: "credential unavailable", auth: true}
	}
	client := &http.Client{CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	response, err := client.Do(request)
	request.Header.Del("Authorization")
	if err != nil {
		code := "unavailable"
		if errors.Is(err, context.DeadlineExceeded) || errors.Is(command.Err(), context.DeadlineExceeded) {
			code = "timeout"
		}
		if mutation {
			code = "outcome_uncertain"
		}
		return &kernelCallFailure{code: code, stage: stage, detail: "request failed", uncertain: mutation}
	}
	defer response.Body.Close()
	if response.StatusCode == http.StatusUnauthorized || response.StatusCode == http.StatusForbidden {
		_ = s.database.MarkProviderAuthenticationFailed(ctx, accountID, revision, time.Now())
		return &kernelCallFailure{code: "unauthenticated", stage: stage, detail: "credential rejected", auth: true}
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		code := "unavailable"
		if response.StatusCode == http.StatusNotFound {
			code = "session_not_found"
		} else if response.StatusCode == http.StatusRequestTimeout || response.StatusCode == http.StatusGatewayTimeout {
			code = "timeout"
		}
		uncertain := mutation && response.StatusCode >= 500
		if uncertain {
			code = "outcome_uncertain"
		}
		return &kernelCallFailure{code: code, stage: stage, detail: "HTTP " + strconv.Itoa(response.StatusCode), uncertain: uncertain}
	}
	if target == nil {
		return nil
	}
	raw, err := io.ReadAll(io.LimitReader(response.Body, kernelResponseLimit+1))
	if err != nil || len(raw) > kernelResponseLimit || json.Unmarshal(raw, target) != nil {
		return &kernelCallFailure{code: browserFailureCodeForMutation(mutation), stage: stage, detail: "response was invalid", uncertain: mutation}
	}
	return nil
}

func browserFailureCodeForMutation(mutation bool) string {
	if mutation {
		return "outcome_uncertain"
	}
	return "unavailable"
}

func (s *Service) deleteKernelBrowser(ctx context.Context, session *browserSession) error {
	failure := s.kernelJSON(ctx, session.providerAccountID, session.credentialRevision, http.MethodDelete,
		"/browsers/"+session.kernelID, nil, nil, false, "delete")
	if failure != nil && failure.code != "session_not_found" {
		return errors.New("delete Kernel browser")
	}
	return nil
}

func safeKernelSessionID(value string) bool {
	if value == "" || len(value) > 200 {
		return false
	}
	for _, char := range value {
		if char < utf8.RuneSelf && (char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9' || char == '-' || char == '_') {
			continue
		}
		return false
	}
	return true
}

func kernelBrowserResponse(raw *kernelRawSnapshot, maxChars int) (*browseProviderResponse, error) {
	if !utf8.ValidString(raw.URL) || !utf8.ValidString(raw.Title) || !utf8.ValidString(raw.Text) {
		return nil, errors.New("Kernel browser snapshot is invalid")
	}
	text, textCut := truncateRunes(raw.Text, maxChars)
	title, _ := truncateRunes(raw.Title, 500)
	elementCut := len(raw.Elements) > 200
	elements := raw.Elements
	if elementCut {
		elements = elements[:200]
	}
	for index := range elements {
		if !utf8.ValidString(elements[index].Role) || !utf8.ValidString(elements[index].Name) {
			return nil, errors.New("Kernel browser snapshot is invalid")
		}
		elements[index].Role, _ = truncateRunes(elements[index].Role, 100)
		elements[index].Name, _ = truncateRunes(elements[index].Name, 500)
	}
	snapshot := &browseSnapshot{URL: raw.URL, Title: title, Text: text, Revision: 1, Elements: elements, Truncated: textCut || elementCut}
	response := &browseProviderResponse{Provider: kernelProvider, State: "open", Snapshot: snapshot}
	if raw.Screenshot != "" {
		decoded, err := base64.StdEncoding.DecodeString(raw.Screenshot)
		if err != nil || len(decoded) > kernelScreenshotLimit || raw.Width < 1 || raw.Height < 1 {
			return nil, errors.New("Kernel browser screenshot is invalid")
		}
		response.Screenshot = &browseScreenshot{MediaType: "image/png", Data: raw.Screenshot, Width: raw.Width, Height: raw.Height}
	}
	return response, nil
}

func truncateRunes(value string, limit int) (string, bool) {
	if limit < 0 {
		limit = 0
	}
	if utf8.RuneCountInString(value) <= limit {
		return value, false
	}
	runes := []rune(value)
	return string(runes[:limit]), true
}

func (s *Service) browserUpload(ctx context.Context, owner string, arguments map[string]any) (artifact.File, string, string, error) {
	if s.artifacts == nil {
		return artifact.File{}, "", "", errors.New("Task artifact upload is unavailable")
	}
	taskID, ok := taskIDFromBrowserOwner(owner)
	artifactID, artifactOK := arguments["artifact_id"].(string)
	versionID, versionOK := arguments["artifact_version_id"].(string)
	if !ok || !artifactOK || !versionOK {
		return artifact.File{}, "", "", errors.New("use one exact local artifact version owned by the current Task")
	}
	value, version, err := s.database.ArtifactVersionWithArtifact(ctx, versionID)
	wantOwner := store.ArtifactOwner{ObjectType: "task", ObjectID: taskID}
	if err != nil || value.Artifact.ID != artifactID || value.Artifact.Owner != wantOwner || value.Artifact.StorageKind != store.ArtifactLocalFile {
		return artifact.File{}, "", "", errors.New("use one exact local artifact version owned by the current Task")
	}
	file, err := s.artifacts.Read(value.Artifact, version)
	if err != nil {
		return artifact.File{}, "", "", errors.New("Task artifact content is unavailable")
	}
	if len(file.Bytes) > kernelUploadLimit {
		return artifact.File{}, "", "", errors.New("choose a Task artifact smaller than 256 KiB")
	}
	return file, artifactID, versionID, nil
}

func taskIDFromBrowserOwner(owner string) (string, bool) {
	if !strings.HasPrefix(owner, "task:") {
		return "", false
	}
	separator := strings.LastIndexByte(owner, ':')
	if separator <= len("task:") || separator == len(owner)-1 {
		return "", false
	}
	generation, err := strconv.ParseInt(owner[separator+1:], 10, 64)
	return strings.TrimPrefix(owner[:separator], "task:"), err == nil && generation > 0
}

func kernelScript(name string, arguments map[string]any, upload *browserUploadFile) (string, error) {
	switch name {
	case BrowseOpenName:
		wait := arguments["wait_until"]
		if wait == "networkidle0" {
			wait = "networkidle"
		}
		return kernelRequestGuard + "\nawait page.goto(" + jsValue(arguments["url"]) + ", {waitUntil:" + jsValue(wait) + "});\nawait page.waitForTimeout(250);\n" + kernelSnapshotScript, nil
	case BrowseSnapshotName:
		return kernelRequestGuard + "\n" + kernelSnapshotScript, nil
	case BrowseInteractName:
		return kernelInteractionScript(arguments, upload)
	case BrowseWaitName:
		return kernelWaitScript(arguments), nil
	case BrowseHistoryName:
		return kernelHistoryScript(arguments), nil
	default:
		return "", errors.New("Kernel browser tool is unavailable")
	}
}

func jsValue(value any) string {
	encoded, _ := json.Marshal(value)
	return string(encoded)
}

func kernelInteractionScript(arguments map[string]any, upload *browserUploadFile) (string, error) {
	action, _ := arguments["action"].(string)
	operation := map[string]string{
		"click": "await locator.click();", "fill": "await locator.fill(value);", "type": "await locator.type(value);",
		"press_key":     "const keys={enter:'Enter',backspace:'Backspace',arrowup:'ArrowUp',arrowdown:'ArrowDown',arrowleft:'ArrowLeft',arrowright:'ArrowRight',escape:'Escape',tab:'Tab',delete:'Delete',home:'Home',end:'End',pageup:'PageUp',pagedown:'PageDown'}; await locator.press(keys[value.toLowerCase()]||value);",
		"select_option": "await locator.selectOption(value);", "upload_file": "await locator.setInputFiles(upload);",
	}[action]
	if operation == "" || action == "upload_file" && upload == nil {
		return "", errors.New("Kernel browser interaction is invalid")
	}
	uploadValue := "null"
	if upload != nil {
		uploadValue = "{name:" + jsValue(upload.Filename) + ",mimeType:" + jsValue(upload.MediaType) + ",buffer:Buffer.from(" +
			jsValue(base64.StdEncoding.EncodeToString(upload.Bytes)) + ",'base64')}"
	}
	return fmt.Sprintf(`%s
const reference=%s;
const value=%s;
const upload=%s;
if(!/^e\d{1,3}$/.test(reference)) return {ok:false,element_found:false};
const locator=page.locator('[data-noema-ref="'+reference+'"]');
if(await locator.count()!==1) return {ok:false,element_found:false};
let mainDocumentStatus=null;
const recordMainDocument=response=>{const request=response.request();if(request.isNavigationRequest()&&request.frame()===page.mainFrame())mainDocumentStatus=response.status();};
page.on('response',recordMainDocument);
try{%s await page.waitForTimeout(250);}catch(_){return {ok:false,element_found:true};}finally{page.off('response',recordMainDocument);}
%s`, kernelRequestGuard, jsValue(arguments["ref"]), jsValue(arguments["value"]), uploadValue, operation, kernelSnapshotScript), nil
}

func kernelWaitScript(arguments map[string]any) string {
	condition, _ := arguments["condition"].(map[string]any)
	text, hasText := condition["text"]
	statement := "await page.waitForFunction(reference=>Array.from(document.querySelectorAll('[data-noema-ref]')).some(element=>element.dataset.noemaRef===reference),reference,{timeout:timeoutMs});"
	if hasText {
		statement = "await page.waitForFunction(text=>Boolean(document.body&&document.body.innerText.includes(text)),text,{timeout:timeoutMs});"
	}
	return kernelRequestGuard + "\nconst text=" + jsValue(text) + ";const reference=" + jsValue(condition["ref"]) + ";const timeoutMs=" +
		jsValue(arguments["timeout_ms"]) + ";\n" + statement + "\n" + kernelSnapshotScript
}

func kernelHistoryScript(arguments map[string]any) string {
	action, _ := arguments["action"].(string)
	operation := map[string]string{"back": "page.goBack({waitUntil:'load'})", "forward": "page.goForward({waitUntil:'load'})", "reload": "page.reload({waitUntil:'load'})"}[action]
	history := action != "reload"
	return kernelRequestGuard + "\nconst response=await " + operation + ";if(!response&&" + strconv.FormatBool(history) +
		")return {ok:false,history_available:false};await page.waitForTimeout(250);\n" + kernelSnapshotScript
}

const kernelRequestGuard = `
await context.unroute('**/*').catch(()=>{});
await context.route('**/*',async route=>{
  const raw=route.request().url();let allowed=false;
  try{
    const parsed=new URL(raw);
    if(['data:','blob:','about:','chrome-extension:'].includes(parsed.protocol))allowed=true;
    if(['http:','https:','ws:','wss:'].includes(parsed.protocol)){
      const host=parsed.hostname.toLowerCase().replace(/^\[|\]$/g,'').replace(/\.$/,'');
      const parts=host.split('.').map(Number);
      const ipv4=parts.length===4&&parts.every(Number.isInteger)&&parts.every(part=>part>=0&&part<=255);
      const privateIpv4=ipv4&&(parts[0]===0||parts[0]===10||(parts[0]===100&&parts[1]>=64&&parts[1]<=127)||parts[0]===127||(parts[0]===169&&parts[1]===254)||(parts[0]===172&&parts[1]>=16&&parts[1]<=31)||(parts[0]===192&&parts[1]===0)||(parts[0]===192&&parts[1]===168)||(parts[0]===198&&parts[1]>=18&&parts[1]<=19)||parts[0]>=224);
      const privateIpv6=host==='::'||host==='::1'||host.startsWith('::ffff:')||host.startsWith('100:')||host.startsWith('2001:db8:')||host.startsWith('2002:')||host.startsWith('64:ff9b:')||host.startsWith('fc')||host.startsWith('fd')||/^fe[89ab]/.test(host)||host.startsWith('ff');
      const blockedName=host==='localhost'||host.endsWith('.localhost')||host.endsWith('.local')||host.endsWith('.internal')||host.endsWith('.test')||host.endsWith('.invalid')||host.endsWith('.example');
      allowed=!blockedName&&!privateIpv4&&!(host.includes(':')&&privateIpv6);
    }
  }catch(_){}
  if(allowed)await route.continue();else await route.abort();
});`

const kernelSnapshotScript = `
const collectSnapshot=async()=>{
  let snapshot=null;
  for(let attempt=0;attempt<3;attempt+=1){
    try{
      snapshot=await page.evaluate(()=>{
        const body=document.body?document.body.cloneNode(true):null;
        if(body)body.querySelectorAll('noscript,script,style,template').forEach(element=>element.remove());
        const nodes=Array.from(document.querySelectorAll('a[href],button,input,textarea,select,[role="button"],[tabindex]'));
        const elements=nodes.map((element,index)=>{
          const reference='e'+(index+1);element.dataset.noemaRef=reference;
          const name=element.getAttribute('aria-label')||element.innerText||element.value||element.getAttribute('placeholder')||'';
          const form=element.form||(element.closest&&element.closest('form'));let submission=null;
          if(form&&!['button','reset'].includes(String(element.type||'').toLowerCase())){
            const fields=[];let omittedControlCount=0;
            for(const control of Array.from(form.elements)){
              const fieldName=String(control.name||'');if(!fieldName||control.disabled)continue;
              const type=String(control.type||'').toLowerCase();
              if(['hidden','password','file'].includes(type)){omittedControlCount+=1;continue;}
              if(['button','reset'].includes(type))continue;
              if((type==='checkbox'||type==='radio')&&!control.checked)continue;
              if((control.tagName==='BUTTON'||type==='submit'||type==='image')&&control!==element)continue;
              const values=control.tagName==='SELECT'&&control.multiple?Array.from(control.selectedOptions).map(option=>option.value):[control.value];
              for(const value of values)fields.push({name:fieldName,value:String(value||'')});
            }
            submission={destination:String(element.formAction||form.action||window.location.href),method:String(element.formMethod||form.method||'get'),fields:fields.slice(0,64),omitted_control_count:omittedControlCount,truncated:fields.length>64};
          }
          return {reference,role:element.getAttribute('role')||element.tagName.toLowerCase(),name:String(name).trim(),href:element.href||null,disabled:Boolean(element.disabled||element.getAttribute('aria-disabled')==='true'),submission};
        });
        return {url:window.location.href,title:String(document.title),text:String(body?body.innerText:''),elements,width:Number(window.innerWidth)||0,height:Number(window.innerHeight)||0};
      });break;
    }catch(error){if(attempt===2)throw error;await page.waitForLoadState('domcontentloaded',{timeout:5000}).catch(()=>{});await page.waitForTimeout(250);}
  }
  let screenshot=null;try{screenshot=(await page.screenshot({type:'png'})).toString('base64');}catch(_){}
  return {...snapshot,screenshot};
};
return {ok:true,snapshot:await collectSnapshot(),main_document_status:typeof mainDocumentStatus==='number'?mainDocumentStatus:null};`
