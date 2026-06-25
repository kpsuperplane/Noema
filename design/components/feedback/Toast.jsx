import React from 'react';

/* Noema Toast — transient notification. Render one inside a fixed container. */

const CSS = `
.noema-toast{
  display:flex; align-items:flex-start; gap:11px; width:340px; max-width:90vw;
  padding:13px 14px; background:var(--surface-card); border:1px solid var(--border-subtle);
  border-radius:var(--radius-md); box-shadow:var(--shadow-lg); font-family:var(--font-sans);
  animation:noema-toast-in var(--dur-base) var(--ease-spring);
}
.noema-toast__icon{ flex:none; width:20px; height:20px; display:flex; align-items:center; justify-content:center; margin-top:1px; }
.noema-toast--success .noema-toast__icon{ color:var(--status-success); }
.noema-toast--warning .noema-toast__icon{ color:var(--status-warning); }
.noema-toast--danger .noema-toast__icon{ color:var(--status-danger); }
.noema-toast--info .noema-toast__icon{ color:var(--status-info); }
.noema-toast__body{ flex:1; min-width:0; }
.noema-toast__title{ font-size:var(--text-sm); font-weight:var(--weight-semibold); color:var(--text-primary); }
.noema-toast__msg{ margin-top:2px; font-size:var(--text-xs); color:var(--text-secondary); line-height:var(--leading-normal); }
.noema-toast__x{ flex:none; width:22px; height:22px; border:none; background:transparent; color:var(--text-muted); cursor:pointer; border-radius:var(--radius-sm); display:flex; align-items:center; justify-content:center; }
.noema-toast__x:hover{ background:var(--surface-hover); color:var(--text-primary); }
.noema-toast__stack{ position:fixed; z-index:1100; bottom:20px; right:20px; display:flex; flex-direction:column; gap:10px; }
@keyframes noema-toast-in{ from{ opacity:0; transform:translateY(10px); } }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'toast');
  el.textContent = CSS;
  document.head.appendChild(el);
}

const ICONS = {
  success: <path d="M3 10.5l4 4 8-9" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" fill="none" />,
  warning: <path d="M10 3l8 14H2L10 3zM10 8v4M10 14.5v.5" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" fill="none" />,
  danger: <path d="M10 3a7 7 0 100 14 7 7 0 000-14zM10 6v5M10 13.5v.5" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" fill="none" />,
  info: <path d="M10 3a7 7 0 100 14 7 7 0 000-14zM10 9v5M10 6.5v.5" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" fill="none" />,
};

export function Toast({ title, message, tone = 'info', onClose, className = '', ...rest }) {
  ensureStyles();
  return (
    <div className={`noema-toast noema-toast--${tone} ${className}`} role="status" {...rest}>
      <span className="noema-toast__icon"><svg width="20" height="20" viewBox="0 0 20 20">{ICONS[tone]}</svg></span>
      <div className="noema-toast__body">
        {title && <div className="noema-toast__title">{title}</div>}
        {message && <div className="noema-toast__msg">{message}</div>}
      </div>
      {onClose && (
        <button className="noema-toast__x" aria-label="Dismiss" onClick={onClose}>
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none"><path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
        </button>
      )}
    </div>
  );
}
