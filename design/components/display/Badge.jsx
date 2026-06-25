import React from 'react';

/* Noema Badge — small status/label pill. */

const CSS = `
.noema-badge{
  display:inline-flex; align-items:center; gap:5px; height:22px; padding:0 9px;
  font-family:var(--font-sans); font-size:var(--text-xs); font-weight:var(--weight-semibold);
  line-height:1; letter-spacing:0; border-radius:var(--radius-pill); white-space:nowrap;
  border:1px solid transparent;
}
.noema-badge--dot::before{ content:""; width:6px; height:6px; border-radius:50%; background:currentColor; flex:none; }
.noema-badge--neutral{ background:var(--surface-sunken); color:var(--text-secondary); border-color:var(--border-subtle); }
.noema-badge--brand{ background:var(--surface-brand-soft); color:var(--pine-700); }
.noema-badge--accent{ background:var(--surface-accent-soft); color:var(--clay-700); }
.noema-badge--success{ background:var(--status-success-soft); color:var(--pine-700); }
.noema-badge--warning{ background:var(--status-warning-soft); color:var(--amber-700); }
.noema-badge--danger{ background:var(--status-danger-soft); color:var(--red-700); }
.noema-badge--info{ background:var(--status-info-soft); color:var(--blue-700); }
.noema-badge--solid{ background:var(--action-primary); color:var(--paper-50); }
.noema-badge--outline{ background:transparent; color:var(--text-secondary); border-color:var(--border-default); }
.noema-badge--mono{ font-family:var(--font-mono); font-size:var(--text-2xs); letter-spacing:0.02em; }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'badge');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Badge({ children, tone = 'neutral', dot = false, mono = false, className = '', ...rest }) {
  ensureStyles();
  const cls = [
    'noema-badge',
    `noema-badge--${tone}`,
    dot && 'noema-badge--dot',
    mono && 'noema-badge--mono',
    className,
  ].filter(Boolean).join(' ');
  return <span className={cls} {...rest}>{children}</span>;
}
