package webtool

func obscuraInteractionScript(arguments map[string]any) string {
	operation := map[string]string{
		"click": `
            element.focus();
            emit(new MouseEvent('mousedown', {bubbles:true,cancelable:true,button:0,buttons:1}));
            emit(new MouseEvent('mouseup', {bubbles:true,cancelable:true,button:0,buttons:0}));
            const tag = element.tagName;
            const type = String(element.getAttribute('type') || '').toLowerCase();
            const checkable = tag === 'INPUT' && (type === 'checkbox' || type === 'radio');
            const oldChecked = checkable ? Boolean(element.checked) : false;
            if (checkable) element.checked = type === 'radio' ? true : !oldChecked;
            const accepted = emit(new MouseEvent('click', {bubbles:true,cancelable:true,button:0,buttons:0}));
            if (!accepted && checkable) element.checked = oldChecked;
            if (accepted && checkable && element.checked !== oldChecked) {
              emit(new Event('input', {bubbles:true}));
              emit(new Event('change', {bubbles:true}));
            } else if (accepted) {
              const link = element.closest ? element.closest('a[href]') : null;
              if (link) {
                const href = link.getAttribute('href');
                if (href && !href.startsWith('#') && !href.startsWith('javascript:')) location.assign(href);
              } else if ((tag === 'BUTTON' && type !== 'button' && type !== 'reset') ||
                         (tag === 'INPUT' && (type === 'submit' || type === 'image'))) {
                const form = element.form || (element.closest && element.closest('form'));
                if (form) form.requestSubmit ? form.requestSubmit(element) : form.submit();
              }
            }
            `,
		"fill": `
            element.focus();
            setValue(value);
            emit(new Event('input', {bubbles:true}));
            emit(new Event('change', {bubbles:true}));
            `,
		"type": `
            element.focus();
            for (const character of value) {
              const accepted = emit(new KeyboardEvent('keydown', {key:character,bubbles:true,cancelable:true}));
              if (accepted) {
                setValue(String(element.value || '') + character);
                emit(new Event('input', {bubbles:true}));
              }
              emit(new KeyboardEvent('keyup', {key:character,bubbles:true}));
            }
            `,
		"press_key": `
            element.focus();
            const namedKeys = {enter:'Enter',backspace:'Backspace',arrowup:'ArrowUp',arrowdown:'ArrowDown',arrowleft:'ArrowLeft',arrowright:'ArrowRight',escape:'Escape',tab:'Tab',delete:'Delete',home:'Home',end:'End',pageup:'PageUp',pagedown:'PageDown'};
            const key = namedKeys[value.toLowerCase()] || value;
            const accepted = emit(new KeyboardEvent('keydown', {key,bubbles:true,cancelable:true}));
            if (accepted && key === 'Backspace') {
              setValue(String(element.value || '').slice(0, -1));
              emit(new Event('input', {bubbles:true}));
            } else if (accepted && key === 'Enter') {
              if (element.tagName === 'TEXTAREA') {
                setValue(String(element.value || '') + '\n');
                emit(new Event('input', {bubbles:true}));
              } else {
                const form = element.form || (element.closest && element.closest('form'));
                if (form) form.requestSubmit ? form.requestSubmit() : form.submit();
              }
            }
            emit(new KeyboardEvent('keyup', {key,bubbles:true}));
            `,
		"select_option": `
            element.focus();
            setValue(value);
            emit(new Event('input', {bubbles:true}));
            emit(new Event('change', {bubbles:true}));
            `,
	}[arguments["action"].(string)]
	return "(() => { const ref = " + jsValue(arguments["ref"]) + "; const value = " + jsValue(arguments["value"]) + `; const element = Array.from(document.querySelectorAll('[data-noema-ref]')).find(item => item.dataset.noemaRef === ref); if (!element) return false; const emit = event => element.dispatchEvent(globalThis.__obscura_markTrusted(event)); const setValue = next => { globalThis.__obscura_setFieldValue(element, 'value', next); if (element.setSelectionRange) element.setSelectionRange(String(next).length, String(next).length); }; ` + operation + " return true; })()"
}
