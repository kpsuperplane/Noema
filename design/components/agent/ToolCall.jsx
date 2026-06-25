import React from 'react';

/* Noema ToolCall — a collapsible record of an agent invoking a tool.
   Mono tool name, status pill, expandable args/result. Core to Noema's
   "show your work" transparency principle. */

const CSS = `
.noema-tool{
  border:1px solid var(--border-subtle); border-radius:var(--radius-md);
  background:var(--surface-card); box-shadow:var(--shadow-xs); overflow:hidden;
  font-family:var(--font-sans); max-width:760px;
}
.noema-tool__head{
  display:flex; align-items:center; gap:10px; width:100%; padding:10px 12px;
  background:transparent; border:none; cursor:pointer; text-align:left;
  transition: background var(--dur-fast) var(--ease-out);
}
.noema-tool__head:hover{ background:var(--surface-hover); }
.noema-tool__glyph{ flex:none; width:26px; height:26px; border-radius:var(--radius-sm); display:flex; align-items:center; justify-content:center; background:var(--surface-sunken); color:var(--text-secondary); }
.noema-tool__name{ flex:1; min-width:0; font-family:var(--font-mono); font-size:var(--text-xs); font-weight:var(--weight-medium); color:var(--text-primary); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.noema-tool__name b{ color:var(--text-brand); font-weight:var(--weight-semibold); }
.noema-tool__status{
  flex:none; display:inline-flex; align-items:center; gap:5px; height:20px; padding:0 8px;
  font-size:var(--text-2xs); font-weight:var(--weight-semibold); border-radius:var(--radius-pill);
  font-family:var(--font-mono); letter-spacing:0.02em;
}
.noema-tool__status--running{ background:var(--status-info-soft); color:var(--blue-700); }
.noema-tool__status--success{ background:var(--status-success-soft); color:var(--pine-700); }
.noema-tool__status--error{ background:var(--status-danger-soft); color:var(--red-700); }
.noema-tool__status--pending{ background:var(--surface-sunken); color:var(--text-muted); }
.noema-tool__spin{ width:8px; height:8px; border-radius:50%; border:1.5px solid currentColor; border-top-color:transparent; animation:noema-spin 0.7s linear infinite; }
.noema-tool__dot{ width:6px; height:6px; border-radius:50%; background:currentColor; }
.noema-tool__chev{ flex:none; color:var(--text-faint); transition: transform var(--dur-base) var(--ease-out); }
.noema-tool__chev--open{ transform:rotate(90deg); }
.noema-tool__body{ border-top:1px solid var(--border-subtle); }
.noema-tool__section{ padding:10px 12px; }
.noema-tool__section + .noema-tool__section{ border-top:1px solid var(--border-subtle); }
.noema-tool__label{ font-family:var(--font-mono); font-size:var(--text-2xs); text-transform:uppercase; letter-spacing:var(--tracking-wider); color:var(--text-faint); margin-bottom:6px; }
.noema-tool__code{ margin:0; font-family:var(--font-mono); font-size:var(--text-xs); line-height:1.55; color:var(--text-secondary); white-space:pre-wrap; word-break:break-word; }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'toolcall');
  el.textContent = CSS;
  document.head.appendChild(el);
}

const STATUS_LABEL = { running: 'Running', success: 'Done', error: 'Failed', pending: 'Queued' };

export function ToolCall({
  tool, status = 'success', args, result, defaultOpen = false, icon, className = '', ...rest
}) {
  ensureStyles();
  const [open, setOpen] = React.useState(defaultOpen);
  const hasBody = args != null || result != null;
  return (
    <div className={`noema-tool ${className}`} {...rest}>
      <button className="noema-tool__head" onClick={() => hasBody && setOpen((o) => !o)} aria-expanded={open}>
        <span className="noema-tool__glyph">
          {icon || (
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none"><path d="M6.5 2.5L3 6l3.5 3.5M9.5 2.5L13 6l-3.5 3.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>
          )}
        </span>
        <span className="noema-tool__name">Called <b>{tool}</b></span>
        <span className={`noema-tool__status noema-tool__status--${status}`}>
          {status === 'running' ? <span className="noema-tool__spin" /> : <span className="noema-tool__dot" />}
          {STATUS_LABEL[status]}
        </span>
        {hasBody && (
          <svg className={`noema-tool__chev ${open ? 'noema-tool__chev--open' : ''}`} width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M5 3l4 4-4 4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></svg>
        )}
      </button>
      {open && hasBody && (
        <div className="noema-tool__body">
          {args != null && (
            <div className="noema-tool__section">
              <div className="noema-tool__label">Arguments</div>
              <pre className="noema-tool__code">{typeof args === 'string' ? args : JSON.stringify(args, null, 2)}</pre>
            </div>
          )}
          {result != null && (
            <div className="noema-tool__section">
              <div className="noema-tool__label">Result</div>
              <pre className="noema-tool__code">{typeof result === 'string' ? result : JSON.stringify(result, null, 2)}</pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
