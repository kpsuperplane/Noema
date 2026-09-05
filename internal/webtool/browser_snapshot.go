package webtool

import (
	"encoding/base64"
	"errors"
	"unicode/utf8"
)

type browserRawSnapshot struct {
	URL, Title, Text string
	Elements         []browseElement
	Screenshot       string `json:"screenshot"`
	Width, Height    int
}

func browserSnapshotResponse(raw *browserRawSnapshot, maxChars int, provider string) (*browseProviderResponse, error) {
	if !utf8.ValidString(raw.URL) || !utf8.ValidString(raw.Title) || !utf8.ValidString(raw.Text) {
		return nil, errors.New("browser snapshot is invalid")
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
			return nil, errors.New("browser snapshot is invalid")
		}
		elements[index].Role, _ = truncateRunes(elements[index].Role, 100)
		elements[index].Name, _ = truncateRunes(elements[index].Name, 500)
	}
	snapshot := &browseSnapshot{URL: raw.URL, Title: title, Text: text, Revision: 1, Elements: elements, Truncated: textCut || elementCut}
	response := &browseProviderResponse{Provider: provider, State: "open", Snapshot: snapshot}
	if raw.Screenshot != "" {
		decoded, err := base64.StdEncoding.DecodeString(raw.Screenshot)
		if err == nil && len(decoded) <= browserScreenshotLimit && raw.Width > 0 && raw.Height > 0 {
			response.Screenshot = &browseScreenshot{MediaType: "image/png", Data: raw.Screenshot, Width: raw.Width, Height: raw.Height}
		}
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

const browserScreenshotLimit = 900_000

const browserSnapshotScript = `()=>{
        const clip=(value,limit)=>Array.from(String(value)).slice(0,limit).join('');
        const body=document.body?document.body.cloneNode(true):null;
        if(body)body.querySelectorAll('noscript,script,style,template').forEach(element=>element.remove());
        const nodes=Array.from(document.querySelectorAll('a[href],button,input,textarea,select,[role="button"],[tabindex]')).slice(0,201);
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
              for(const value of values)fields.push({name:clip(fieldName,256),value:clip(value||'',1000)});
            }
            submission={destination:String(element.formAction||form.action||window.location.href),method:String(element.formMethod||form.method||'get'),fields:fields.slice(0,64),omitted_control_count:omittedControlCount,truncated:fields.length>64};
          }
          return {reference,role:clip(element.getAttribute('role')||element.tagName.toLowerCase(),101),name:clip(String(name).trim(),501),href:element.href||null,disabled:Boolean(element.disabled||element.getAttribute('aria-disabled')==='true'),submission};
        });
        return {url:window.location.href,title:clip(document.title,501),text:clip(body?body.innerText:'',20001),elements,width:Number(window.innerWidth)||0,height:Number(window.innerHeight)||0};
}`
