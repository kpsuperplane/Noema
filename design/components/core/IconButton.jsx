import React from 'react';

/* Noema IconButton — square/circular control holding a single icon. */

const CSS = `
.noema-iconbtn{
  --_s: 40px;
  display:inline-flex; align-items:center; justify-content:center;
  width:var(--_s); height:var(--_s); padding:0; flex:none;
  border-radius:var(--radius-md); border:1px solid transparent; cursor:pointer;
  color:var(--text-secondary); background:transparent;
  transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out),
              border-color var(--dur-fast) var(--ease-out), transform var(--dur-fast) var(--ease-out);
}
.noema-iconbtn:active{ transform: scale(0.92); }
.noema-iconbtn:focus-visible{ outline:none; box-shadow:var(--ring); }
.noema-iconbtn[disabled]{ opacity:0.4; cursor:not-allowed; }
.noema-iconbtn--sm{ --_s:32px; border-radius:var(--radius-sm); }
.noema-iconbtn--lg{ --_s:48px; border-radius:var(--radius-lg); }
.noema-iconbtn--round{ border-radius:var(--radius-round); }

.noema-iconbtn--ghost:hover{ background:var(--surface-hover); color:var(--text-primary); }
.noema-iconbtn--soft{ background:var(--surface-sunken); color:var(--text-primary); }
.noema-iconbtn--soft:hover{ background:var(--surface-active); }
.noema-iconbtn--outline{ border-color:var(--border-default); color:var(--text-primary); }
.noema-iconbtn--outline:hover{ background:var(--surface-hover); border-color:var(--border-strong); }
.noema-iconbtn--primary{ background:var(--action-primary); color:var(--action-primary-text); }
.noema-iconbtn--primary:hover{ background:var(--action-primary-hover); }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'iconbutton');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function IconButton({
  children,
  icon,
  variant = 'ghost',
  size = 'md',
  round = false,
  'aria-label': ariaLabel,
  className = '',
  ...rest
}) {
  ensureStyles();
  const cls = [
    'noema-iconbtn',
    `noema-iconbtn--${variant}`,
    size !== 'md' && `noema-iconbtn--${size}`,
    round && 'noema-iconbtn--round',
    className,
  ].filter(Boolean).join(' ');
  return (
    <button className={cls} aria-label={ariaLabel} {...rest}>
      {icon || children}
    </button>
  );
}
