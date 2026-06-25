/* @ds-bundle: {"format":3,"namespace":"NoemaDesignSystem_3d237e","components":[{"name":"ChatMessage","sourcePath":"components/agent/ChatMessage.jsx"},{"name":"ToolCall","sourcePath":"components/agent/ToolCall.jsx"},{"name":"Button","sourcePath":"components/core/Button.jsx"},{"name":"IconButton","sourcePath":"components/core/IconButton.jsx"},{"name":"Avatar","sourcePath":"components/display/Avatar.jsx"},{"name":"Badge","sourcePath":"components/display/Badge.jsx"},{"name":"Card","sourcePath":"components/display/Card.jsx"},{"name":"Tag","sourcePath":"components/display/Tag.jsx"},{"name":"Dialog","sourcePath":"components/feedback/Dialog.jsx"},{"name":"Toast","sourcePath":"components/feedback/Toast.jsx"},{"name":"Tooltip","sourcePath":"components/feedback/Tooltip.jsx"},{"name":"Checkbox","sourcePath":"components/forms/Checkbox.jsx"},{"name":"Input","sourcePath":"components/forms/Input.jsx"},{"name":"Select","sourcePath":"components/forms/Select.jsx"},{"name":"Switch","sourcePath":"components/forms/Switch.jsx"},{"name":"Textarea","sourcePath":"components/forms/Textarea.jsx"},{"name":"Tabs","sourcePath":"components/navigation/Tabs.jsx"}],"sourceHashes":{"components/agent/ChatMessage.jsx":"397d4f9f9b06","components/agent/ToolCall.jsx":"b6240a9ed70d","components/core/Button.jsx":"9327a7518bc9","components/core/IconButton.jsx":"3c2084fcb793","components/display/Avatar.jsx":"ae3b091d9dd3","components/display/Badge.jsx":"0316d4f3818a","components/display/Card.jsx":"cfe9d0855810","components/display/Tag.jsx":"fd28269eca64","components/feedback/Dialog.jsx":"6ea96b3ee08b","components/feedback/Toast.jsx":"dabf6587181c","components/feedback/Tooltip.jsx":"7b95e739d43e","components/forms/Checkbox.jsx":"2183ada8c746","components/forms/Input.jsx":"7c7829bba2d2","components/forms/Select.jsx":"80329c6fd180","components/forms/Switch.jsx":"5fcd8c512d73","components/forms/Textarea.jsx":"59d4d6668a0e","components/navigation/Tabs.jsx":"8943f5f6a61c","spec/Gallery.jsx":"ae49d247cd97"},"inlinedExternals":[],"unexposedExports":[]} */

(() => {

const __ds_ns = (window.NoemaDesignSystem_3d237e = window.NoemaDesignSystem_3d237e || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/agent/ChatMessage.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema ChatMessage — a turn in an agent conversation.
   User turns sit in a soft bubble; agent turns are full-width prose with a mark. */

const CSS = `
.noema-msg{ display:flex; gap:12px; font-family:var(--font-sans); padding:var(--space-3) 0; }
.noema-msg--user{ flex-direction:row-reverse; }
.noema-msg__avatar{ flex:none; }
.noema-msg__col{ min-width:0; max-width:760px; }
.noema-msg--user .noema-msg__col{ display:flex; flex-direction:column; align-items:flex-end; }
.noema-msg__meta{ display:flex; align-items:baseline; gap:8px; margin-bottom:4px; }
.noema-msg__name{ font-size:var(--text-sm); font-weight:var(--weight-semibold); color:var(--text-primary); }
.noema-msg__time{ font-family:var(--font-mono); font-size:var(--text-2xs); color:var(--text-faint); }
.noema-msg__bubble{
  font-size:var(--text-base); line-height:var(--leading-relaxed); color:var(--text-primary);
}
.noema-msg--user .noema-msg__bubble{
  background:var(--surface-brand-soft); border:1px solid var(--pine-100);
  padding:11px 15px; border-radius:var(--radius-lg) var(--radius-lg) var(--radius-xs) var(--radius-lg);
  font-size:var(--text-sm); text-align:left;
}
.noema-msg--agent .noema-msg__bubble > :first-child{ margin-top:0; }
.noema-msg--agent .noema-msg__bubble > :last-child{ margin-bottom:0; }
.noema-msg__typing{ display:inline-flex; gap:4px; padding:6px 0; }
.noema-msg__typing span{ width:6px; height:6px; border-radius:50%; background:var(--pine-400); animation:noema-typing 1.2s var(--ease-in-out) infinite; }
.noema-msg__typing span:nth-child(2){ animation-delay:0.15s; }
.noema-msg__typing span:nth-child(3){ animation-delay:0.3s; }
@keyframes noema-typing{ 0%,60%,100%{ opacity:0.3; transform:translateY(0); } 30%{ opacity:1; transform:translateY(-3px); } }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'chatmessage');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function ChatMessage({
  role = 'agent',
  name,
  time,
  avatar,
  typing = false,
  children,
  className = '',
  ...rest
}) {
  ensureStyles();
  const isUser = role === 'user';
  return /*#__PURE__*/React.createElement("div", _extends({
    className: `noema-msg noema-msg--${isUser ? 'user' : 'agent'} ${className}`
  }, rest), avatar && /*#__PURE__*/React.createElement("div", {
    className: "noema-msg__avatar"
  }, avatar), /*#__PURE__*/React.createElement("div", {
    className: "noema-msg__col"
  }, (name || time) && /*#__PURE__*/React.createElement("div", {
    className: "noema-msg__meta"
  }, name && /*#__PURE__*/React.createElement("span", {
    className: "noema-msg__name"
  }, name), time && /*#__PURE__*/React.createElement("span", {
    className: "noema-msg__time"
  }, time)), /*#__PURE__*/React.createElement("div", {
    className: "noema-msg__bubble"
  }, typing ? /*#__PURE__*/React.createElement("span", {
    className: "noema-msg__typing",
    "aria-label": "Agent is typing"
  }, /*#__PURE__*/React.createElement("span", null), /*#__PURE__*/React.createElement("span", null), /*#__PURE__*/React.createElement("span", null)) : children)));
}
Object.assign(__ds_scope, { ChatMessage });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/agent/ChatMessage.jsx", error: String((e && e.message) || e) }); }

// components/agent/ToolCall.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
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
const STATUS_LABEL = {
  running: 'Running',
  success: 'Done',
  error: 'Failed',
  pending: 'Queued'
};
function ToolCall({
  tool,
  status = 'success',
  args,
  result,
  defaultOpen = false,
  icon,
  className = '',
  ...rest
}) {
  ensureStyles();
  const [open, setOpen] = React.useState(defaultOpen);
  const hasBody = args != null || result != null;
  return /*#__PURE__*/React.createElement("div", _extends({
    className: `noema-tool ${className}`
  }, rest), /*#__PURE__*/React.createElement("button", {
    className: "noema-tool__head",
    onClick: () => hasBody && setOpen(o => !o),
    "aria-expanded": open
  }, /*#__PURE__*/React.createElement("span", {
    className: "noema-tool__glyph"
  }, icon || /*#__PURE__*/React.createElement("svg", {
    width: "14",
    height: "14",
    viewBox: "0 0 16 16",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M6.5 2.5L3 6l3.5 3.5M9.5 2.5L13 6l-3.5 3.5",
    stroke: "currentColor",
    strokeWidth: "1.5",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  }))), /*#__PURE__*/React.createElement("span", {
    className: "noema-tool__name"
  }, "Called ", /*#__PURE__*/React.createElement("b", null, tool)), /*#__PURE__*/React.createElement("span", {
    className: `noema-tool__status noema-tool__status--${status}`
  }, status === 'running' ? /*#__PURE__*/React.createElement("span", {
    className: "noema-tool__spin"
  }) : /*#__PURE__*/React.createElement("span", {
    className: "noema-tool__dot"
  }), STATUS_LABEL[status]), hasBody && /*#__PURE__*/React.createElement("svg", {
    className: `noema-tool__chev ${open ? 'noema-tool__chev--open' : ''}`,
    width: "14",
    height: "14",
    viewBox: "0 0 14 14",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M5 3l4 4-4 4",
    stroke: "currentColor",
    strokeWidth: "1.6",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  }))), open && hasBody && /*#__PURE__*/React.createElement("div", {
    className: "noema-tool__body"
  }, args != null && /*#__PURE__*/React.createElement("div", {
    className: "noema-tool__section"
  }, /*#__PURE__*/React.createElement("div", {
    className: "noema-tool__label"
  }, "Arguments"), /*#__PURE__*/React.createElement("pre", {
    className: "noema-tool__code"
  }, typeof args === 'string' ? args : JSON.stringify(args, null, 2))), result != null && /*#__PURE__*/React.createElement("div", {
    className: "noema-tool__section"
  }, /*#__PURE__*/React.createElement("div", {
    className: "noema-tool__label"
  }, "Result"), /*#__PURE__*/React.createElement("pre", {
    className: "noema-tool__code"
  }, typeof result === 'string' ? result : JSON.stringify(result, null, 2)))));
}
Object.assign(__ds_scope, { ToolCall });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/agent/ToolCall.jsx", error: String((e && e.message) || e) }); }

// components/core/Button.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
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
function Button({
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
  const cls = ['noema-btn', `noema-btn--${variant}`, size !== 'md' && `noema-btn--${size}`, fullWidth && 'noema-btn--full', className].filter(Boolean).join(' ');
  const extra = {};
  if (Tag === 'button') extra.disabled = disabled || loading;else if (disabled || loading) extra['aria-disabled'] = 'true';
  return /*#__PURE__*/React.createElement(Tag, _extends({
    className: cls
  }, extra, rest), loading && /*#__PURE__*/React.createElement("span", {
    className: "noema-btn__spin",
    "aria-hidden": "true"
  }), !loading && leftIcon, children, !loading && rightIcon);
}
Object.assign(__ds_scope, { Button });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Button.jsx", error: String((e && e.message) || e) }); }

// components/core/IconButton.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
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
function IconButton({
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
  const cls = ['noema-iconbtn', `noema-iconbtn--${variant}`, size !== 'md' && `noema-iconbtn--${size}`, round && 'noema-iconbtn--round', className].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement("button", _extends({
    className: cls,
    "aria-label": ariaLabel
  }, rest), icon || children);
}
Object.assign(__ds_scope, { IconButton });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/IconButton.jsx", error: String((e && e.message) || e) }); }

// components/display/Avatar.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Avatar — user/agent identity. Supports image, initials, or agent mark.
   Agent avatars use a pine tint to distinguish them from people. */

const CSS = `
.noema-avatar{
  position:relative; display:inline-flex; align-items:center; justify-content:center;
  flex:none; font-family:var(--font-sans); font-weight:var(--weight-semibold);
  color:var(--paper-50); overflow:hidden; user-select:none;
  border-radius:var(--radius-round);
}
.noema-avatar--square{ border-radius:var(--radius-md); }
.noema-avatar--xs{ width:24px; height:24px; font-size:10px; }
.noema-avatar--sm{ width:32px; height:32px; font-size:12px; }
.noema-avatar--md{ width:40px; height:40px; font-size:15px; }
.noema-avatar--lg{ width:52px; height:52px; font-size:19px; }
.noema-avatar__img{ width:100%; height:100%; object-fit:cover; }
.noema-avatar--agent{ background:linear-gradient(150deg, var(--pine-500), var(--pine-700)); }
.noema-avatar--person{ background:var(--clay-500); }
.noema-avatar--ring{ box-shadow:0 0 0 2px var(--surface-card), 0 0 0 4px var(--pine-300); }
.noema-avatar__status{
  position:absolute; right:-1px; bottom:-1px; width:30%; height:30%; min-width:8px; min-height:8px;
  border-radius:50%; border:2px solid var(--surface-card); background:var(--status-success);
}
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'avatar');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function initials(name = '') {
  return name.trim().split(/\s+/).slice(0, 2).map(w => w[0]).join('').toUpperCase() || '?';
}
function Avatar({
  name,
  src,
  kind = 'person',
  size = 'md',
  square = false,
  ring = false,
  status = false,
  className = '',
  ...rest
}) {
  ensureStyles();
  const cls = ['noema-avatar', `noema-avatar--${size}`, `noema-avatar--${kind}`, square && 'noema-avatar--square', ring && 'noema-avatar--ring', className].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement("span", _extends({
    className: cls,
    title: name
  }, rest), src ? /*#__PURE__*/React.createElement("img", {
    className: "noema-avatar__img",
    src: src,
    alt: name || ''
  }) : initials(name), status && /*#__PURE__*/React.createElement("span", {
    className: "noema-avatar__status"
  }));
}
Object.assign(__ds_scope, { Avatar });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/display/Avatar.jsx", error: String((e && e.message) || e) }); }

// components/display/Badge.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
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
function Badge({
  children,
  tone = 'neutral',
  dot = false,
  mono = false,
  className = '',
  ...rest
}) {
  ensureStyles();
  const cls = ['noema-badge', `noema-badge--${tone}`, dot && 'noema-badge--dot', mono && 'noema-badge--mono', className].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement("span", _extends({
    className: cls
  }, rest), children);
}
Object.assign(__ds_scope, { Badge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/display/Badge.jsx", error: String((e && e.message) || e) }); }

// components/display/Card.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Card — the primary content container. White surface, hairline border,
   soft warm shadow. Optional `interactive` lift on hover. */

const CSS = `
.noema-card{
  display:block; background:var(--surface-card); color:var(--text-primary);
  border:1px solid var(--border-subtle); border-radius:var(--radius-lg);
  box-shadow:var(--shadow-sm); overflow:clip;
}
.noema-card--pad-sm{ padding:var(--space-4); }
.noema-card--pad-md{ padding:var(--space-6); }
.noema-card--pad-lg{ padding:var(--space-8); }
.noema-card--flat{ box-shadow:none; }
.noema-card--raised{ box-shadow:var(--shadow-md); }
.noema-card--sunken{ background:var(--surface-sunken); box-shadow:none; border-color:var(--border-subtle); }
.noema-card--brand{ background:var(--surface-brand-soft); border-color:var(--pine-100); }
.noema-card--interactive{ cursor:pointer; transition: transform var(--dur-base) var(--ease-out), box-shadow var(--dur-base) var(--ease-out), border-color var(--dur-base) var(--ease-out); }
.noema-card--interactive:hover{ transform: translateY(-2px); box-shadow:var(--shadow-lg); border-color:var(--border-default); }
.noema-card--interactive:active{ transform: translateY(0); box-shadow:var(--shadow-sm); }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'card');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function Card({
  children,
  padding = 'md',
  variant = 'default',
  interactive = false,
  as = 'div',
  className = '',
  ...rest
}) {
  ensureStyles();
  const Tag = as;
  const cls = ['noema-card', padding && `noema-card--pad-${padding}`, variant !== 'default' && `noema-card--${variant}`, interactive && 'noema-card--interactive', className].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement(Tag, _extends({
    className: cls
  }, rest), children);
}
Object.assign(__ds_scope, { Card });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/display/Card.jsx", error: String((e && e.message) || e) }); }

// components/display/Tag.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Tag — input chip / filter token. Optional leading icon and remove button. */

const CSS = `
.noema-tag{
  display:inline-flex; align-items:center; gap:6px; height:26px; padding:0 10px;
  font-family:var(--font-sans); font-size:var(--text-xs); font-weight:var(--weight-medium);
  color:var(--text-primary); background:var(--surface-sunken);
  border:1px solid var(--border-subtle); border-radius:var(--radius-sm); white-space:nowrap;
}
.noema-tag--brand{ background:var(--surface-brand-soft); border-color:var(--pine-100); color:var(--pine-700); }
.noema-tag--mono{ font-family:var(--font-mono); font-size:var(--text-2xs); }
.noema-tag--selectable{ cursor:pointer; transition: background var(--dur-fast) var(--ease-out), border-color var(--dur-fast) var(--ease-out); }
.noema-tag--selectable:hover{ border-color:var(--border-default); background:var(--surface-active); }
.noema-tag--selected{ background:var(--action-primary); border-color:var(--action-primary); color:var(--paper-50); }
.noema-tag__x{
  display:inline-flex; align-items:center; justify-content:center; width:15px; height:15px;
  margin-right:-3px; border-radius:50%; border:none; background:transparent; color:inherit;
  cursor:pointer; opacity:0.6;
}
.noema-tag__x:hover{ opacity:1; background:rgba(0,0,0,0.08); }
.noema-tag__x svg{ width:9px; height:9px; }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'tag');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function Tag({
  children,
  icon,
  tone = 'neutral',
  mono = false,
  selected,
  selectable = false,
  onRemove,
  className = '',
  ...rest
}) {
  ensureStyles();
  const cls = ['noema-tag', tone === 'brand' && 'noema-tag--brand', mono && 'noema-tag--mono', (selectable || selected) && 'noema-tag--selectable', selected && 'noema-tag--selected', className].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement("span", _extends({
    className: cls
  }, rest), icon, children, onRemove && /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: "noema-tag__x",
    "aria-label": "Remove",
    onClick: onRemove
  }, /*#__PURE__*/React.createElement("svg", {
    viewBox: "0 0 10 10",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M1.5 1.5l7 7M8.5 1.5l-7 7",
    stroke: "currentColor",
    strokeWidth: "1.5",
    strokeLinecap: "round"
  }))));
}
Object.assign(__ds_scope, { Tag });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/display/Tag.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Dialog.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
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
function Dialog({
  open = true,
  onClose,
  title,
  description,
  size = 'md',
  icon,
  footer,
  children,
  className = '',
  ...rest
}) {
  ensureStyles();
  if (!open) return null;
  const sizeCls = size !== 'md' ? `noema-dialog--${size}` : '';
  return /*#__PURE__*/React.createElement("div", {
    className: "noema-dialog__scrim",
    onMouseDown: e => {
      if (e.target === e.currentTarget) onClose && onClose();
    }
  }, /*#__PURE__*/React.createElement("div", _extends({
    className: `noema-dialog ${sizeCls} ${className}`,
    role: "dialog",
    "aria-modal": "true",
    "aria-label": typeof title === 'string' ? title : undefined
  }, rest), /*#__PURE__*/React.createElement("div", {
    className: "noema-dialog__head"
  }, icon, /*#__PURE__*/React.createElement("div", {
    className: "noema-dialog__titles"
  }, title && /*#__PURE__*/React.createElement("h2", {
    className: "noema-dialog__title"
  }, title), description && /*#__PURE__*/React.createElement("p", {
    className: "noema-dialog__desc"
  }, description)), onClose && /*#__PURE__*/React.createElement("button", {
    className: "noema-dialog__x",
    "aria-label": "Close",
    onClick: onClose
  }, /*#__PURE__*/React.createElement("svg", {
    width: "14",
    height: "14",
    viewBox: "0 0 14 14",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M2 2l10 10M12 2L2 12",
    stroke: "currentColor",
    strokeWidth: "1.6",
    strokeLinecap: "round"
  })))), children && /*#__PURE__*/React.createElement("div", {
    className: "noema-dialog__body"
  }, children), footer && /*#__PURE__*/React.createElement("div", {
    className: "noema-dialog__foot"
  }, footer)));
}
Object.assign(__ds_scope, { Dialog });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Dialog.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Toast.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
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
  success: /*#__PURE__*/React.createElement("path", {
    d: "M3 10.5l4 4 8-9",
    stroke: "currentColor",
    strokeWidth: "1.8",
    strokeLinecap: "round",
    strokeLinejoin: "round",
    fill: "none"
  }),
  warning: /*#__PURE__*/React.createElement("path", {
    d: "M10 3l8 14H2L10 3zM10 8v4M10 14.5v.5",
    stroke: "currentColor",
    strokeWidth: "1.7",
    strokeLinecap: "round",
    strokeLinejoin: "round",
    fill: "none"
  }),
  danger: /*#__PURE__*/React.createElement("path", {
    d: "M10 3a7 7 0 100 14 7 7 0 000-14zM10 6v5M10 13.5v.5",
    stroke: "currentColor",
    strokeWidth: "1.7",
    strokeLinecap: "round",
    fill: "none"
  }),
  info: /*#__PURE__*/React.createElement("path", {
    d: "M10 3a7 7 0 100 14 7 7 0 000-14zM10 9v5M10 6.5v.5",
    stroke: "currentColor",
    strokeWidth: "1.7",
    strokeLinecap: "round",
    fill: "none"
  })
};
function Toast({
  title,
  message,
  tone = 'info',
  onClose,
  className = '',
  ...rest
}) {
  ensureStyles();
  return /*#__PURE__*/React.createElement("div", _extends({
    className: `noema-toast noema-toast--${tone} ${className}`,
    role: "status"
  }, rest), /*#__PURE__*/React.createElement("span", {
    className: "noema-toast__icon"
  }, /*#__PURE__*/React.createElement("svg", {
    width: "20",
    height: "20",
    viewBox: "0 0 20 20"
  }, ICONS[tone])), /*#__PURE__*/React.createElement("div", {
    className: "noema-toast__body"
  }, title && /*#__PURE__*/React.createElement("div", {
    className: "noema-toast__title"
  }, title), message && /*#__PURE__*/React.createElement("div", {
    className: "noema-toast__msg"
  }, message)), onClose && /*#__PURE__*/React.createElement("button", {
    className: "noema-toast__x",
    "aria-label": "Dismiss",
    onClick: onClose
  }, /*#__PURE__*/React.createElement("svg", {
    width: "12",
    height: "12",
    viewBox: "0 0 12 12",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M2 2l8 8M10 2l-8 8",
    stroke: "currentColor",
    strokeWidth: "1.5",
    strokeLinecap: "round"
  }))));
}
Object.assign(__ds_scope, { Toast });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Toast.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Tooltip.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Tooltip — hover/focus label on a wrapped trigger. CSS-driven. */

const CSS = `
.noema-tip{ position:relative; display:inline-flex; }
.noema-tip__pop{
  position:absolute; z-index:1200; left:50%; transform:translateX(-50%) translateY(4px);
  bottom:calc(100% + 8px); padding:6px 9px; white-space:nowrap;
  background:var(--ink-900); color:var(--paper-50); font-family:var(--font-sans);
  font-size:var(--text-xs); font-weight:var(--weight-medium); border-radius:var(--radius-sm);
  box-shadow:var(--shadow-md); opacity:0; pointer-events:none;
  transition: opacity var(--dur-fast) var(--ease-out), transform var(--dur-fast) var(--ease-out);
}
.noema-tip__pop::after{
  content:""; position:absolute; top:100%; left:50%; transform:translateX(-50%);
  border:4px solid transparent; border-top-color:var(--ink-900);
}
.noema-tip:hover .noema-tip__pop, .noema-tip:focus-within .noema-tip__pop{
  opacity:1; transform:translateX(-50%) translateY(0);
}
.noema-tip--bottom .noema-tip__pop{ bottom:auto; top:calc(100% + 8px); transform:translateX(-50%) translateY(-4px); }
.noema-tip--bottom .noema-tip__pop::after{ top:auto; bottom:100%; border-top-color:transparent; border-bottom-color:var(--ink-900); }
.noema-tip--bottom:hover .noema-tip__pop{ transform:translateX(-50%) translateY(0); }
.noema-tip__kbd{ margin-left:6px; font-family:var(--font-mono); font-size:var(--text-2xs); opacity:0.7; }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'tooltip');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function Tooltip({
  label,
  kbd,
  side = 'top',
  children,
  className = '',
  ...rest
}) {
  ensureStyles();
  return /*#__PURE__*/React.createElement("span", _extends({
    className: `noema-tip ${side === 'bottom' ? 'noema-tip--bottom' : ''} ${className}`
  }, rest), children, /*#__PURE__*/React.createElement("span", {
    className: "noema-tip__pop",
    role: "tooltip"
  }, label, kbd && /*#__PURE__*/React.createElement("span", {
    className: "noema-tip__kbd"
  }, kbd)));
}
Object.assign(__ds_scope, { Tooltip });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Tooltip.jsx", error: String((e && e.message) || e) }); }

// components/forms/Checkbox.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Checkbox — accessible custom checkbox with label. */

const CSS = `
.noema-check{ display:inline-flex; align-items:flex-start; gap:10px; cursor:pointer; font-family:var(--font-sans); }
.noema-check--disabled{ opacity:0.5; cursor:not-allowed; }
.noema-check__input{ position:absolute; opacity:0; width:0; height:0; }
.noema-check__box{
  flex:none; width:18px; height:18px; margin-top:1px; border-radius:5px;
  border:1.5px solid var(--border-strong); background:var(--surface-card);
  display:flex; align-items:center; justify-content:center; color:transparent;
  transition: background var(--dur-fast) var(--ease-out), border-color var(--dur-fast) var(--ease-out);
}
.noema-check__input:checked + .noema-check__box{ background:var(--action-primary); border-color:var(--action-primary); color:var(--paper-50); }
.noema-check__input:focus-visible + .noema-check__box{ box-shadow:var(--ring); }
.noema-check__box svg{ width:12px; height:12px; }
.noema-check__label{ font-size:var(--text-sm); color:var(--text-primary); line-height:1.4; }
.noema-check__desc{ display:block; font-size:var(--text-xs); color:var(--text-muted); margin-top:1px; }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'checkbox');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function Checkbox({
  label,
  description,
  disabled,
  id,
  className = '',
  ...rest
}) {
  ensureStyles();
  const fieldId = id || rest.name;
  return /*#__PURE__*/React.createElement("label", {
    className: `noema-check ${disabled ? 'noema-check--disabled' : ''} ${className}`,
    htmlFor: fieldId
  }, /*#__PURE__*/React.createElement("input", _extends({
    id: fieldId,
    type: "checkbox",
    className: "noema-check__input",
    disabled: disabled
  }, rest)), /*#__PURE__*/React.createElement("span", {
    className: "noema-check__box",
    "aria-hidden": "true"
  }, /*#__PURE__*/React.createElement("svg", {
    viewBox: "0 0 12 12",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M2 6.2l2.6 2.6L10 3.2",
    stroke: "currentColor",
    strokeWidth: "1.8",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  }))), label && /*#__PURE__*/React.createElement("span", {
    className: "noema-check__label"
  }, label, description && /*#__PURE__*/React.createElement("span", {
    className: "noema-check__desc"
  }, description)));
}
Object.assign(__ds_scope, { Checkbox });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Checkbox.jsx", error: String((e && e.message) || e) }); }

// components/forms/Input.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Input — labelled text field with hint/error and optional icons. */

const CSS = `
.noema-field{ display:flex; flex-direction:column; gap:6px; font-family:var(--font-sans); }
.noema-field__label{ font-size:var(--text-sm); font-weight:var(--weight-semibold); color:var(--text-primary); }
.noema-field__req{ color:var(--status-danger); margin-left:2px; }
.noema-field__hint{ font-size:var(--text-xs); color:var(--text-muted); }
.noema-field__hint--error{ color:var(--status-danger); }

.noema-input{
  display:flex; align-items:center; gap:8px; height:40px; padding:0 12px;
  background:var(--surface-card); border:1px solid var(--border-default);
  border-radius:var(--radius-md); box-shadow:var(--shadow-xs);
  transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
}
.noema-input:hover{ border-color:var(--border-strong); }
.noema-input:focus-within{ border-color:var(--border-focus); box-shadow:var(--ring); }
.noema-input--sm{ height:32px; border-radius:var(--radius-sm); }
.noema-input--lg{ height:48px; border-radius:var(--radius-lg); }
.noema-input--error{ border-color:var(--status-danger); }
.noema-input--error:focus-within{ box-shadow:0 0 0 3px var(--status-danger-soft); }
.noema-input--disabled{ background:var(--surface-sunken); opacity:0.7; pointer-events:none; }
.noema-input__icon{ display:flex; color:var(--text-muted); flex:none; }
.noema-input__el{
  flex:1; min-width:0; border:none; outline:none; background:transparent;
  font-family:var(--font-sans); font-size:var(--text-sm); color:var(--text-primary);
}
.noema-input__el::placeholder{ color:var(--text-faint); }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'input');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function Input({
  label,
  hint,
  error,
  required,
  size = 'md',
  leftIcon,
  rightIcon,
  disabled,
  id,
  className = '',
  ...rest
}) {
  ensureStyles();
  const fieldId = id || (label ? `noema-${String(label).replace(/\s+/g, '-').toLowerCase()}` : undefined);
  const boxCls = ['noema-input', size !== 'md' && `noema-input--${size}`, error && 'noema-input--error', disabled && 'noema-input--disabled'].filter(Boolean).join(' ');
  return /*#__PURE__*/React.createElement("div", {
    className: `noema-field ${className}`
  }, label && /*#__PURE__*/React.createElement("label", {
    className: "noema-field__label",
    htmlFor: fieldId
  }, label, required && /*#__PURE__*/React.createElement("span", {
    className: "noema-field__req"
  }, "*")), /*#__PURE__*/React.createElement("div", {
    className: boxCls
  }, leftIcon && /*#__PURE__*/React.createElement("span", {
    className: "noema-input__icon"
  }, leftIcon), /*#__PURE__*/React.createElement("input", _extends({
    id: fieldId,
    className: "noema-input__el",
    disabled: disabled,
    "aria-invalid": !!error
  }, rest)), rightIcon && /*#__PURE__*/React.createElement("span", {
    className: "noema-input__icon"
  }, rightIcon)), (error || hint) && /*#__PURE__*/React.createElement("span", {
    className: `noema-field__hint ${error ? 'noema-field__hint--error' : ''}`
  }, error || hint));
}
Object.assign(__ds_scope, { Input });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Input.jsx", error: String((e && e.message) || e) }); }

// components/forms/Select.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Select — styled native <select> with chevron + label/hint. */

const CSS = `
.noema-select__box{ position:relative; display:flex; align-items:center; }
.noema-select__el{
  appearance:none; -webkit-appearance:none; width:100%; height:40px;
  padding:0 38px 0 12px; background:var(--surface-card); color:var(--text-primary);
  font-family:var(--font-sans); font-size:var(--text-sm); cursor:pointer;
  border:1px solid var(--border-default); border-radius:var(--radius-md); box-shadow:var(--shadow-xs);
  transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
}
.noema-select__el:hover{ border-color:var(--border-strong); }
.noema-select__el:focus-visible{ outline:none; border-color:var(--border-focus); box-shadow:var(--ring); }
.noema-select__el:disabled{ background:var(--surface-sunken); opacity:0.7; cursor:not-allowed; }
.noema-select__el--sm{ height:32px; border-radius:var(--radius-sm); font-size:var(--text-xs); }
.noema-select__chev{
  position:absolute; right:12px; pointer-events:none; color:var(--text-muted);
  width:16px; height:16px;
}
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'select');
  el.textContent = CSS;
  document.head.appendChild(el);
}
const Chevron = () => /*#__PURE__*/React.createElement("svg", {
  className: "noema-select__chev",
  viewBox: "0 0 16 16",
  fill: "none",
  "aria-hidden": "true"
}, /*#__PURE__*/React.createElement("path", {
  d: "M4 6l4 4 4-4",
  stroke: "currentColor",
  strokeWidth: "1.6",
  strokeLinecap: "round",
  strokeLinejoin: "round"
}));
function Select({
  label,
  hint,
  error,
  required,
  size = 'md',
  options,
  children,
  id,
  className = '',
  ...rest
}) {
  ensureStyles();
  const fieldId = id || (label ? `noema-sel-${String(label).replace(/\s+/g, '-').toLowerCase()}` : undefined);
  return /*#__PURE__*/React.createElement("div", {
    className: `noema-field ${className}`
  }, label && /*#__PURE__*/React.createElement("label", {
    className: "noema-field__label",
    htmlFor: fieldId
  }, label, required && /*#__PURE__*/React.createElement("span", {
    className: "noema-field__req"
  }, "*")), /*#__PURE__*/React.createElement("div", {
    className: "noema-select__box"
  }, /*#__PURE__*/React.createElement("select", _extends({
    id: fieldId,
    className: `noema-select__el ${size === 'sm' ? 'noema-select__el--sm' : ''}`
  }, rest), options ? options.map(o => {
    const opt = typeof o === 'string' ? {
      value: o,
      label: o
    } : o;
    return /*#__PURE__*/React.createElement("option", {
      key: opt.value,
      value: opt.value
    }, opt.label);
  }) : children), /*#__PURE__*/React.createElement(Chevron, null)), (error || hint) && /*#__PURE__*/React.createElement("span", {
    className: `noema-field__hint ${error ? 'noema-field__hint--error' : ''}`
  }, error || hint));
}
Object.assign(__ds_scope, { Select });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Select.jsx", error: String((e && e.message) || e) }); }

// components/forms/Switch.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Switch — on/off toggle for settings. */

const CSS = `
.noema-switch{ display:inline-flex; align-items:center; gap:10px; cursor:pointer; font-family:var(--font-sans); }
.noema-switch--disabled{ opacity:0.5; cursor:not-allowed; }
.noema-switch__input{ position:absolute; opacity:0; width:0; height:0; }
.noema-switch__track{
  position:relative; width:38px; height:22px; border-radius:var(--radius-pill);
  background:var(--border-strong); flex:none;
  transition: background var(--dur-base) var(--ease-out);
}
.noema-switch__thumb{
  position:absolute; top:2px; left:2px; width:18px; height:18px; border-radius:50%;
  background:var(--paper-50); box-shadow:var(--shadow-sm);
  transition: transform var(--dur-base) var(--ease-spring);
}
.noema-switch__input:checked + .noema-switch__track{ background:var(--action-primary); }
.noema-switch__input:checked + .noema-switch__track .noema-switch__thumb{ transform: translateX(16px); }
.noema-switch__input:focus-visible + .noema-switch__track{ box-shadow:var(--ring); }
.noema-switch__label{ font-size:var(--text-sm); color:var(--text-primary); }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'switch');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function Switch({
  label,
  disabled,
  id,
  className = '',
  ...rest
}) {
  ensureStyles();
  const fieldId = id || rest.name;
  return /*#__PURE__*/React.createElement("label", {
    className: `noema-switch ${disabled ? 'noema-switch--disabled' : ''} ${className}`,
    htmlFor: fieldId
  }, /*#__PURE__*/React.createElement("input", _extends({
    id: fieldId,
    type: "checkbox",
    role: "switch",
    className: "noema-switch__input",
    disabled: disabled
  }, rest)), /*#__PURE__*/React.createElement("span", {
    className: "noema-switch__track",
    "aria-hidden": "true"
  }, /*#__PURE__*/React.createElement("span", {
    className: "noema-switch__thumb"
  })), label && /*#__PURE__*/React.createElement("span", {
    className: "noema-switch__label"
  }, label));
}
Object.assign(__ds_scope, { Switch });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Switch.jsx", error: String((e && e.message) || e) }); }

// components/forms/Textarea.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Textarea — multi-line field, shares the field shell with Input. */

const CSS = `
.noema-ta__box{
  display:block; padding:10px 12px; background:var(--surface-card);
  border:1px solid var(--border-default); border-radius:var(--radius-md);
  box-shadow:var(--shadow-xs);
  transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
}
.noema-ta__box:hover{ border-color:var(--border-strong); }
.noema-ta__box:focus-within{ border-color:var(--border-focus); box-shadow:var(--ring); }
.noema-ta__box--error{ border-color:var(--status-danger); }
.noema-ta__el{
  display:block; width:100%; border:none; outline:none; background:transparent; resize:vertical;
  font-family:var(--font-sans); font-size:var(--text-sm); line-height:var(--leading-normal);
  color:var(--text-primary); min-height:84px;
}
.noema-ta__el::placeholder{ color:var(--text-faint); }
.noema-ta__el--mono{ font-family:var(--font-mono); font-size:var(--text-xs); }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'textarea');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function Textarea({
  label,
  hint,
  error,
  required,
  mono = false,
  id,
  className = '',
  rows = 4,
  ...rest
}) {
  ensureStyles();
  const fieldId = id || (label ? `noema-ta-${String(label).replace(/\s+/g, '-').toLowerCase()}` : undefined);
  return /*#__PURE__*/React.createElement("div", {
    className: `noema-field ${className}`
  }, label && /*#__PURE__*/React.createElement("label", {
    className: "noema-field__label",
    htmlFor: fieldId
  }, label, required && /*#__PURE__*/React.createElement("span", {
    className: "noema-field__req"
  }, "*")), /*#__PURE__*/React.createElement("div", {
    className: `noema-ta__box ${error ? 'noema-ta__box--error' : ''}`
  }, /*#__PURE__*/React.createElement("textarea", _extends({
    id: fieldId,
    rows: rows,
    "aria-invalid": !!error,
    className: `noema-ta__el ${mono ? 'noema-ta__el--mono' : ''}`
  }, rest))), (error || hint) && /*#__PURE__*/React.createElement("span", {
    className: `noema-field__hint ${error ? 'noema-field__hint--error' : ''}`
  }, error || hint));
}
Object.assign(__ds_scope, { Textarea });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Textarea.jsx", error: String((e && e.message) || e) }); }

// components/navigation/Tabs.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Noema Tabs — segmented or underline tab bar. Uncontrolled or controlled. */

const CSS = `
.noema-tabs{ font-family:var(--font-sans); }
.noema-tabs__list{ display:inline-flex; align-items:center; gap:2px; }

/* underline style */
.noema-tabs--underline .noema-tabs__list{ gap:18px; border-bottom:1px solid var(--border-subtle); }
.noema-tabs--underline .noema-tab{
  position:relative; padding:9px 1px; background:none; border:none; cursor:pointer;
  font-size:var(--text-sm); font-weight:var(--weight-medium); color:var(--text-muted);
  transition: color var(--dur-fast) var(--ease-out);
}
.noema-tabs--underline .noema-tab:hover{ color:var(--text-secondary); }
.noema-tabs--underline .noema-tab[aria-selected="true"]{ color:var(--text-primary); font-weight:var(--weight-semibold); }
.noema-tabs--underline .noema-tab[aria-selected="true"]::after{
  content:""; position:absolute; left:0; right:0; bottom:-1px; height:2px;
  background:var(--action-primary); border-radius:2px 2px 0 0;
}

/* segmented (pill) style */
.noema-tabs--segmented .noema-tabs__list{
  gap:2px; padding:3px; background:var(--surface-sunken); border-radius:var(--radius-md);
  border:1px solid var(--border-subtle);
}
.noema-tabs--segmented .noema-tab{
  padding:6px 14px; border:none; background:transparent; cursor:pointer; border-radius:var(--radius-sm);
  font-size:var(--text-sm); font-weight:var(--weight-medium); color:var(--text-secondary);
  transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
}
.noema-tabs--segmented .noema-tab:hover{ color:var(--text-primary); }
.noema-tabs--segmented .noema-tab[aria-selected="true"]{
  background:var(--surface-card); color:var(--text-primary); font-weight:var(--weight-semibold);
  box-shadow:var(--shadow-xs);
}
.noema-tab:focus-visible{ outline:none; box-shadow:var(--ring); border-radius:var(--radius-sm); }
.noema-tab__count{ margin-left:6px; font-size:var(--text-2xs); color:var(--text-muted); font-family:var(--font-mono); }
`;
let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'tabs');
  el.textContent = CSS;
  document.head.appendChild(el);
}
function Tabs({
  items = [],
  value,
  defaultValue,
  onChange,
  variant = 'underline',
  className = '',
  ...rest
}) {
  ensureStyles();
  const [internal, setInternal] = React.useState(defaultValue ?? (items[0] && items[0].value));
  const active = value !== undefined ? value : internal;
  const select = v => {
    if (value === undefined) setInternal(v);
    onChange && onChange(v);
  };
  return /*#__PURE__*/React.createElement("div", _extends({
    className: `noema-tabs noema-tabs--${variant} ${className}`
  }, rest), /*#__PURE__*/React.createElement("div", {
    className: "noema-tabs__list",
    role: "tablist"
  }, items.map(it => /*#__PURE__*/React.createElement("button", {
    key: it.value,
    role: "tab",
    "aria-selected": active === it.value,
    className: "noema-tab",
    onClick: () => select(it.value)
  }, it.label, it.count != null && /*#__PURE__*/React.createElement("span", {
    className: "noema-tab__count"
  }, it.count)))));
}
Object.assign(__ds_scope, { Tabs });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/navigation/Tabs.jsx", error: String((e && e.message) || e) }); }

// spec/Gallery.jsx
try { (() => {
/* Live component gallery for the Noema Design System spec page.
   Self-contained: inlines a Lucide-backed Icon and mounts every primitive. */

const DS = window.NoemaDesignSystem_3d237e;
const {
  Button,
  IconButton,
  Input,
  Textarea,
  Select,
  Checkbox,
  Switch,
  Card,
  Badge,
  Avatar,
  Tag,
  Tabs,
  Dialog,
  Toast,
  Tooltip,
  ChatMessage,
  ToolCall
} = DS;
function Icon({
  name,
  size = 18,
  strokeWidth,
  style
}) {
  const node = window.lucide && window.lucide.icons && window.lucide.icons[name];
  if (!node) return null;
  const base = node[1] || {},
    kids = node[2] || [];
  return React.createElement('svg', {
    width: size,
    height: size,
    viewBox: '0 0 24 24',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: strokeWidth || base['stroke-width'] || 2,
    strokeLinecap: 'round',
    strokeLinejoin: 'round',
    style,
    'aria-hidden': 'true'
  }, kids.map((c, i) => React.createElement(c[0], {
    key: i,
    ...c[1]
  })));
}
const Plus = () => /*#__PURE__*/React.createElement(Icon, {
  name: "Plus",
  size: 15
});
const Gear = () => /*#__PURE__*/React.createElement(Icon, {
  name: "Settings",
  size: 17
});
function Block({
  label,
  children,
  span
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "gx-block",
    style: span ? {
      gridColumn: '1 / -1'
    } : null
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-block__label"
  }, label), /*#__PURE__*/React.createElement("div", {
    className: "gx-block__body"
  }, children));
}
function Gallery() {
  const [tab, setTab] = React.useState('chat');
  const [dialogOpen, setDialogOpen] = React.useState(false);
  React.useEffect(() => {
    if (window.lucide) window.lucide.createIcons();
  });
  return /*#__PURE__*/React.createElement("div", {
    className: "gx-grid"
  }, /*#__PURE__*/React.createElement(Block, {
    label: "Button \u2014 variants"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-row"
  }, /*#__PURE__*/React.createElement(Button, {
    variant: "primary"
  }, "Primary"), /*#__PURE__*/React.createElement(Button, {
    variant: "accent"
  }, "Accent"), /*#__PURE__*/React.createElement(Button, {
    variant: "secondary"
  }, "Secondary"), /*#__PURE__*/React.createElement(Button, {
    variant: "ghost"
  }, "Ghost"), /*#__PURE__*/React.createElement(Button, {
    variant: "danger"
  }, "Danger"))), /*#__PURE__*/React.createElement(Block, {
    label: "Button \u2014 sizes, icons & states"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-row"
  }, /*#__PURE__*/React.createElement(Button, {
    size: "sm"
  }, "Small"), /*#__PURE__*/React.createElement(Button, {
    leftIcon: /*#__PURE__*/React.createElement(Plus, null)
  }, "New agent"), /*#__PURE__*/React.createElement(Button, {
    size: "lg",
    variant: "accent"
  }, "Get started"), /*#__PURE__*/React.createElement(Button, {
    loading: true
  }, "Working"), /*#__PURE__*/React.createElement(Button, {
    disabled: true
  }, "Disabled"))), /*#__PURE__*/React.createElement(Block, {
    label: "IconButton"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-row"
  }, /*#__PURE__*/React.createElement(IconButton, {
    "aria-label": "Settings",
    icon: /*#__PURE__*/React.createElement(Gear, null)
  }), /*#__PURE__*/React.createElement(IconButton, {
    "aria-label": "Settings",
    variant: "soft",
    icon: /*#__PURE__*/React.createElement(Gear, null)
  }), /*#__PURE__*/React.createElement(IconButton, {
    "aria-label": "Settings",
    variant: "outline",
    icon: /*#__PURE__*/React.createElement(Gear, null)
  }), /*#__PURE__*/React.createElement(IconButton, {
    "aria-label": "Add",
    variant: "primary",
    icon: /*#__PURE__*/React.createElement(Plus, null)
  }), /*#__PURE__*/React.createElement(IconButton, {
    "aria-label": "Add",
    variant: "primary",
    round: true,
    icon: /*#__PURE__*/React.createElement(Plus, null)
  }))), /*#__PURE__*/React.createElement(Block, {
    label: "Badge & Tag"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-row",
    style: {
      marginBottom: 12
    }
  }, /*#__PURE__*/React.createElement(Badge, {
    tone: "success",
    dot: true
  }, "Online"), /*#__PURE__*/React.createElement(Badge, {
    tone: "brand"
  }, "Open source"), /*#__PURE__*/React.createElement(Badge, {
    tone: "warning"
  }, "Needs review"), /*#__PURE__*/React.createElement(Badge, {
    tone: "danger"
  }, "Failed"), /*#__PURE__*/React.createElement(Badge, {
    tone: "info"
  }, "Beta"), /*#__PURE__*/React.createElement(Badge, {
    tone: "neutral",
    mono: true
  }, "v0.4.2")), /*#__PURE__*/React.createElement("div", {
    className: "gx-row"
  }, /*#__PURE__*/React.createElement(Tag, {
    tone: "brand",
    mono: true,
    icon: /*#__PURE__*/React.createElement(Icon, {
      name: "Hash",
      size: 11
    })
  }, "filesystem"), /*#__PURE__*/React.createElement(Tag, {
    selectable: true,
    selected: true
  }, "web-search"), /*#__PURE__*/React.createElement(Tag, {
    onRemove: () => {}
  }, "shell"))), /*#__PURE__*/React.createElement(Block, {
    label: "Avatar \u2014 agents vs. people"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-row",
    style: {
      alignItems: 'center'
    }
  }, /*#__PURE__*/React.createElement(Avatar, {
    name: "Ada Lovelace",
    status: true
  }), /*#__PURE__*/React.createElement(Avatar, {
    name: "Kit Mercer",
    size: "sm"
  }), /*#__PURE__*/React.createElement(Avatar, {
    kind: "agent",
    square: true,
    name: "Research Agent"
  }), /*#__PURE__*/React.createElement(Avatar, {
    kind: "agent",
    square: true,
    size: "lg",
    name: "WB",
    ring: true
  }))), /*#__PURE__*/React.createElement(Block, {
    label: "Tabs"
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 18
    }
  }, /*#__PURE__*/React.createElement(Tabs, {
    variant: "underline",
    items: [{
      value: 'chat',
      label: 'Chat'
    }, {
      value: 'tools',
      label: 'Tools',
      count: 4
    }, {
      value: 'memory',
      label: 'Memory'
    }],
    value: tab,
    onChange: setTab
  }), /*#__PURE__*/React.createElement(Tabs, {
    variant: "segmented",
    items: [{
      value: 'chat',
      label: 'Chat'
    }, {
      value: 'tools',
      label: 'Tools'
    }, {
      value: 'memory',
      label: 'Memory'
    }],
    value: tab,
    onChange: setTab
  }))), /*#__PURE__*/React.createElement(Block, {
    label: "Inputs"
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 14
    }
  }, /*#__PURE__*/React.createElement(Input, {
    label: "Workspace name",
    defaultValue: "research-agent"
  }), /*#__PURE__*/React.createElement(Input, {
    label: "Search",
    size: "sm",
    leftIcon: /*#__PURE__*/React.createElement(Icon, {
      name: "Search",
      size: 15
    }),
    placeholder: "Filter tools\u2026"
  }), /*#__PURE__*/React.createElement(Select, {
    label: "Model",
    options: ['Claude Sonnet', 'Llama 3 70B', 'Mistral Large']
  }))), /*#__PURE__*/React.createElement(Block, {
    label: "Prompt, toggles & choices"
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 14
    }
  }, /*#__PURE__*/React.createElement(Textarea, {
    label: "System prompt",
    mono: true,
    rows: 3,
    defaultValue: 'You are a careful research assistant.'
  }), /*#__PURE__*/React.createElement(Checkbox, {
    label: "Run trusted tools automatically",
    defaultChecked: true
  }), /*#__PURE__*/React.createElement(Switch, {
    label: "Local-only mode",
    defaultChecked: true
  }))), /*#__PURE__*/React.createElement(Block, {
    label: "Cards"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-cards"
  }, /*#__PURE__*/React.createElement(Card, null, /*#__PURE__*/React.createElement("div", {
    className: "gx-ct"
  }, "Default"), /*#__PURE__*/React.createElement("div", {
    className: "gx-cb"
  }, "White surface, hairline border, soft warm shadow.")), /*#__PURE__*/React.createElement(Card, {
    variant: "brand"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-ct"
  }, "Brand"), /*#__PURE__*/React.createElement("div", {
    className: "gx-cb"
  }, "Pine-tinted highlight surface.")), /*#__PURE__*/React.createElement(Card, {
    variant: "sunken"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-ct"
  }, "Sunken"), /*#__PURE__*/React.createElement("div", {
    className: "gx-cb"
  }, "Recessed panel for nested content.")))), /*#__PURE__*/React.createElement(Block, {
    label: "Feedback \u2014 Dialog \xB7 Toast \xB7 Tooltip"
  }, /*#__PURE__*/React.createElement("div", {
    className: "gx-row",
    style: {
      marginBottom: 14
    }
  }, /*#__PURE__*/React.createElement(Button, {
    variant: "secondary",
    onClick: () => setDialogOpen(true)
  }, "Open dialog"), /*#__PURE__*/React.createElement(Tooltip, {
    label: "New conversation",
    kbd: "\u2318N"
  }, /*#__PURE__*/React.createElement(Button, {
    variant: "ghost"
  }, "Hover for tooltip"))), /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 10,
      maxWidth: 360
    }
  }, /*#__PURE__*/React.createElement(Toast, {
    tone: "success",
    title: "Agent deployed",
    message: "research-agent is now live.",
    onClose: () => {}
  }), /*#__PURE__*/React.createElement(Toast, {
    tone: "warning",
    title: "Tool needs approval",
    message: "shell.exec is awaiting your OK."
  })), dialogOpen && /*#__PURE__*/React.createElement(Dialog, {
    open: true,
    title: "Grant filesystem access?",
    description: "Research Agent is requesting read access to ~/Documents.",
    onClose: () => setDialogOpen(false),
    footer: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(Button, {
      variant: "ghost",
      onClick: () => setDialogOpen(false)
    }, "Not now"), /*#__PURE__*/React.createElement(Button, {
      onClick: () => setDialogOpen(false)
    }, "Allow"))
  }, "Tools can be revoked anytime from Settings \u2192 Permissions.")), /*#__PURE__*/React.createElement(Block, {
    label: "Agent \u2014 ChatMessage & ToolCall",
    span: true
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      maxWidth: 620
    }
  }, /*#__PURE__*/React.createElement(ChatMessage, {
    role: "user",
    name: "You",
    time: "2:14 PM",
    avatar: /*#__PURE__*/React.createElement(Avatar, {
      name: "You",
      size: "sm"
    })
  }, "Find the most-cited paper on retrieval-augmented generation and summarize it."), /*#__PURE__*/React.createElement("div", {
    style: {
      margin: '2px 0 6px 36px',
      display: 'flex',
      flexDirection: 'column',
      gap: 8
    }
  }, /*#__PURE__*/React.createElement(ToolCall, {
    tool: "web_search",
    status: "success",
    args: {
      query: 'most cited RAG paper'
    },
    result: "Lewis et al. 2020"
  }), /*#__PURE__*/React.createElement(ToolCall, {
    tool: "fetch_url",
    status: "running"
  })), /*#__PURE__*/React.createElement(ChatMessage, {
    role: "agent",
    name: "Research Agent",
    time: "2:14 PM",
    avatar: /*#__PURE__*/React.createElement(Avatar, {
      kind: "agent",
      square: true,
      size: "sm",
      name: "RA"
    })
  }, /*#__PURE__*/React.createElement("p", {
    style: {
      margin: 0
    }
  }, "The most-cited work is ", /*#__PURE__*/React.createElement("b", null, "Lewis et\xA0al. (2020)"), " \u2014 it pairs a retriever with a generator so answers stay grounded in sources you can check.")), /*#__PURE__*/React.createElement(ChatMessage, {
    role: "agent",
    typing: true,
    avatar: /*#__PURE__*/React.createElement(Avatar, {
      kind: "agent",
      square: true,
      size: "sm",
      name: "RA"
    })
  }))));
}
ReactDOM.createRoot(document.getElementById('gallery-root')).render(/*#__PURE__*/React.createElement(Gallery, null));
})(); } catch (e) { __ds_ns.__errors.push({ path: "spec/Gallery.jsx", error: String((e && e.message) || e) }); }

__ds_ns.ChatMessage = __ds_scope.ChatMessage;

__ds_ns.ToolCall = __ds_scope.ToolCall;

__ds_ns.Button = __ds_scope.Button;

__ds_ns.IconButton = __ds_scope.IconButton;

__ds_ns.Avatar = __ds_scope.Avatar;

__ds_ns.Badge = __ds_scope.Badge;

__ds_ns.Card = __ds_scope.Card;

__ds_ns.Tag = __ds_scope.Tag;

__ds_ns.Dialog = __ds_scope.Dialog;

__ds_ns.Toast = __ds_scope.Toast;

__ds_ns.Tooltip = __ds_scope.Tooltip;

__ds_ns.Checkbox = __ds_scope.Checkbox;

__ds_ns.Input = __ds_scope.Input;

__ds_ns.Select = __ds_scope.Select;

__ds_ns.Switch = __ds_scope.Switch;

__ds_ns.Textarea = __ds_scope.Textarea;

__ds_ns.Tabs = __ds_scope.Tabs;

})();
