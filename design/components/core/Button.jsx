import React from 'react';

/* Noema Button — primary action surface.
   Self-contained: injects its own namespaced stylesheet once. */

const CSS = `
.noema-btn{
  --_h: 40px; --_px: 18px; --_fs: var(--text-sm);
  display:inline-flex; align-items:center; justify-content:center; gap:8px;
  height:var(--_h); padding:0 var(--_px); font-family:var(--font-sans);
  font-size:var(--_fs); font-weight:var(--weight-semibold); line-height:1;
  letter-spacing:-0.005em; border-radius:var(--radius-md); border:1px solid transparent;
  cursor:pointer; user-select:none; text-decoration:none; white-space:nowrap;
  transition: background var(--dur-fast) var(--ease-out),
              border-color var(--dur-fast) var(--ease-out),
              transform var(--dur-fast) var(--ease-out),
              box-shadow var(--dur-fast) var(--ease-out);
}
.noema-btn:active{ transform: translateY(0.5px) scale(0.985); }
.noema-btn:focus-visible{ outline:none; box-shadow: var(--ring); }
.noema-btn[disabled], .noema-btn[aria-disabled="true"]{ opacity:0.45; cursor:not-allowed; pointer-events:none; }

.noema-btn--sm{ --_h:32px; --_px:13px; --_fs:var(--text-xs); border-radius:var(--radius-sm); }
.noema-btn--lg{ --_h:48px; --_px:24px; --_fs:var(--text-base); border-radius:var(--radius-lg); }
.noema-btn--full{ width:100%; }

.noema-btn--primary{ background:var(--action-primary); color:var(--action-primary-text); box-shadow:var(--shadow-xs); }
.noema-btn--primary:hover{ background:var(--action-primary-hover); }
.noema-btn--primary:active{ background:var(--action-primary-active); }

.noema-btn--accent{ background:var(--action-accent); color:var(--action-accent-text); box-shadow:var(--shadow-xs); }
.noema-btn--accent:hover{ background:var(--action-accent-hover); }
.noema-btn--accent:active{ background:var(--action-accent-active); }

.noema-btn--secondary{ background:var(--surface-card); color:var(--text-primary); border-color:var(--border-default); box-shadow:var(--shadow-xs); }
.noema-btn--secondary:hover{ background:var(--surface-hover); border-color:var(--border-strong); }
.noema-btn--secondary:active{ background:var(--surface-active); }

.noema-btn--ghost{ background:transparent; color:var(--text-primary); }
.noema-btn--ghost:hover{ background:var(--surface-hover); }
.noema-btn--ghost:active{ background:var(--surface-active); }

.noema-btn--danger{ background:var(--status-danger); color:var(--paper-50); box-shadow:var(--shadow-xs); }
.noema-btn--danger:hover{ filter:brightness(0.93); }

.noema-btn__spin{ width:14px; height:14px; border-radius:50%; border:2px solid currentColor; border-top-color:transparent; animation:noema-spin 0.7s linear infinite; }
@keyframes noema-spin{ to{ transform:rotate(360deg); } }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'button');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Button({
  children,
  variant = 'primary',
  size = 'md',
  fullWidth = false,
  loading = false,
  disabled = false,
  leftIcon,
  rightIcon,
  as,
  className = '',
  ...rest
}) {
  ensureStyles();
  const Tag = as || (rest.href ? 'a' : 'button');
  const cls = [
    'noema-btn',
    `noema-btn--${variant}`,
    size !== 'md' && `noema-btn--${size}`,
    fullWidth && 'noema-btn--full',
    className,
  ].filter(Boolean).join(' ');

  const extra = {};
  if (Tag === 'button') extra.disabled = disabled || loading;
  else if (disabled || loading) extra['aria-disabled'] = 'true';

  return (
    <Tag className={cls} {...extra} {...rest}>
      {loading && <span className="noema-btn__spin" aria-hidden="true" />}
      {!loading && leftIcon}
      {children}
      {!loading && rightIcon}
    </Tag>
  );
}
