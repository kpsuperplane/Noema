import React from 'react';

/* Noema Dialog — modal with scrim, card panel, header/body/footer slots. */

const CSS = `
.noema-dialog__scrim{
  position:fixed; inset:0; z-index:1000; display:flex; align-items:center; justify-content:center;
  padding:var(--space-6); background:rgba(23,22,15,0.42); backdrop-filter:blur(3px);
  animation:noema-dlg-fade var(--dur-base) var(--ease-out);
}
.noema-dialog{
  position:relative; width:100%; max-width:480px; max-height:88vh; overflow:auto;
  background:var(--surface-card); border:1px solid var(--border-subtle);
  border-radius:var(--radius-xl); box-shadow:var(--shadow-xl);
  animation:noema-dlg-pop var(--dur-base) var(--ease-spring);
}
.noema-dialog--sm{ max-width:380px; }
.noema-dialog--lg{ max-width:640px; }
.noema-dialog__head{ display:flex; align-items:flex-start; gap:12px; padding:var(--space-6) var(--space-6) var(--space-3); }
.noema-dialog__titles{ flex:1; min-width:0; }
.noema-dialog__title{ font-family:var(--font-display); font-weight:var(--weight-display); font-size:var(--text-lg); letter-spacing:var(--tracking-tight); color:var(--text-primary); margin:0; }
.noema-dialog__desc{ margin:6px 0 0; font-size:var(--text-sm); color:var(--text-secondary); line-height:var(--leading-normal); }
.noema-dialog__x{ flex:none; width:30px; height:30px; display:flex; align-items:center; justify-content:center; border:none; background:transparent; color:var(--text-muted); border-radius:var(--radius-sm); cursor:pointer; }
.noema-dialog__x:hover{ background:var(--surface-hover); color:var(--text-primary); }
.noema-dialog__body{ padding:var(--space-2) var(--space-6) var(--space-4); font-size:var(--text-sm); color:var(--text-secondary); line-height:var(--leading-normal); }
.noema-dialog__foot{ display:flex; align-items:center; justify-content:flex-end; gap:10px; padding:var(--space-4) var(--space-6) var(--space-6); }
@keyframes noema-dlg-fade{ from{ opacity:0; } }
@keyframes noema-dlg-pop{ from{ opacity:0; transform:translateY(8px) scale(0.98); } }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'dialog');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Dialog({
  open = true, onClose, title, description, size = 'md', icon, footer, children, className = '', ...rest
}) {
  ensureStyles();
  if (!open) return null;
  const sizeCls = size !== 'md' ? `noema-dialog--${size}` : '';
  return (
    <div className="noema-dialog__scrim" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose && onClose(); }}>
      <div className={`noema-dialog ${sizeCls} ${className}`} role="dialog" aria-modal="true" aria-label={typeof title === 'string' ? title : undefined} {...rest}>
        <div className="noema-dialog__head">
          {icon}
          <div className="noema-dialog__titles">
            {title && <h2 className="noema-dialog__title">{title}</h2>}
            {description && <p className="noema-dialog__desc">{description}</p>}
          </div>
          {onClose && (
            <button className="noema-dialog__x" aria-label="Close" onClick={onClose}>
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M2 2l10 10M12 2L2 12" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>
            </button>
          )}
        </div>
        {children && <div className="noema-dialog__body">{children}</div>}
        {footer && <div className="noema-dialog__foot">{footer}</div>}
      </div>
    </div>
  );
}
