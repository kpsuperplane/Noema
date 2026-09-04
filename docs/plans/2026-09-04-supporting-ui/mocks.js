/* Static examples only. No requests, credentials, storage, or authentication. */
(() => {
  const button = (label, variant, attributes = '') => window.noemaButtonMarkup[variant]
    .replace('__LABEL__', label).replace('<button ', `<button ${attributes} `);
  const go = (label, target, variant = 'primary', provider = '') =>
    button(label, variant, `data-go="${target}"${provider ? ` data-provider="${provider}"` : ''}`);
  const finish = (label, variant = 'primary') => button(label, variant, 'data-finish');
  const actions = (primary, secondary = '') => `<section class="actions">${secondary}${primary}</section>`;
  const note = text => `<p class="note">${text}</p>`;
  const check = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m5 12 4 4 10-10"/></svg>';
  const status = (title, text = '', kind = '') => `<section class="status ${kind}"><span class="status-icon" aria-hidden="true">${kind === 'success' ? check : kind === 'error' ? '!' : '·'}</span><section><p><strong>${title}</strong></p>${text ? `<p>${text}</p>` : ''}</section></section>`;
  const waiting = text => `<section class="status" role="status"><span class="spinner" aria-hidden="true"></span><p>${text}</p></section>`;
  const field = (id, label, placeholder, error = '') => `<section class="field"><label for="${id}">${label}</label><input id="${id}" type="text" placeholder="${placeholder}" readonly${error ? ` aria-invalid="true" aria-describedby="${id}-error"` : ''}>${error ? `<p class="field-error" id="${id}-error">${error}</p>` : ''}</section>`;
  const facts = rows => `<section class="facts">${rows.map(([label, value]) => `<p class="fact"><span>${label}</span><span>${value}</span></p>`).join('')}</section>`;
  const option = (label, text, target, provider) => `<button type="button" class="option" data-go="${target}" data-provider="${provider}"><span class="option-copy"><strong>${label}</strong><small>${text}</small></span><svg class="option-arrow" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 6 6 6-6 6"/></svg></button>`;
  const recoveryHelp = `<details><summary>Where do I find this code?</summary><p>The current code is in the server’s <code>config.yaml</code> file, under <code>web.recovery_code</code>.</p><p>Ask the person who manages Noema for help.</p></details>`;
  const recoveryRule = note('Each attempt replaces the code, even if it fails.');
  const localFacts = facts([['Model', 'Gemma 4 E4B IT'], ['Download', '5.3 GB'], ['Runs on', 'Your Noema server']]);
  const localDetails = `<details><summary>Model details</summary>${facts([['License', 'Apache-2.0'], ['Model format', 'GGUF · Q4_K_M'], ['Runtime', 'Metal']])}<p>Model file: <code>gemma-4-E4B-it-Q4_K_M.gguf</code></p></details>`;
  const screens = [
    {
      id: 'access-login', group: 'Passkeys', label: 'Sign in', title: 'Unlock Noema',
      intro: 'Use your saved passkey to continue.',
      body: actions(go('Continue with passkey', 'access-prompt'), go('Can’t use your passkey?', 'recovery-code', 'secondary')),
      intent: 'One primary action. Keep the return destination after sign-in.',
      constraint: 'The browser owns the passkey prompt. The mock does not recreate it.'
    },
    {
      id: 'access-create', group: 'Passkeys', label: 'First passkey', title: 'Create your Noema passkey',
      intro: 'Your device will help you save it.',
      body: actions(go('Create passkey', 'provider-choice')),
      intent: 'Create the first passkey before model setup.',
      constraint: 'Initial claim needs no recovery code. This preview skips the browser ceremony.'
    },
    {
      id: 'access-prompt', group: 'Passkeys', label: 'Waiting for passkey', title: 'Unlock Noema',
      intro: 'Use your saved passkey to continue.',
      body: waiting('Complete the passkey prompt to continue.') + actions(go('Cancel', 'access-cancelled', 'secondary')),
      intent: 'Keep the heading and action area stable while the browser prompt is open.',
      constraint: 'Successful sign-in returns to the original destination. Use the state picker to inspect other outcomes.'
    },
    {
      id: 'access-cancelled', group: 'Passkeys', label: 'Prompt cancelled', title: 'Unlock Noema',
      intro: 'The prompt closed or timed out.',
      body: actions(go('Try passkey again', 'access-prompt'), go('Can’t use your passkey?', 'recovery-code', 'secondary')),
      intent: 'Treat cancellation calmly. Keep another attempt easy to find.',
      constraint: 'Keep cancellation distinct from server failure.'
    },
    {
      id: 'access-unsupported', group: 'Passkeys', label: 'Unsupported browser', title: 'Use a browser with passkey support',
      intro: 'This browser cannot use passkeys.',
      body: note('Open this Noema address in a browser that supports passkeys.') + actions(finish('Copy Noema address', 'outline')),
      intent: 'Give a useful next action instead of explaining WebAuthn APIs.',
      constraint: 'Recovery also requires a passkey ceremony. Do not imply that it bypasses browser support.'
    },
    {
      id: 'recovery-code', group: 'Recovery', label: 'Enter code', title: 'Recover access',
      intro: 'Enter a code to create a passkey.',
      body: field('recovery-code-input', 'Recovery code', 'Paste the current code') + recoveryHelp + recoveryRule + actions(go('Continue', 'recovery-enroll'), go('Back to sign in', 'access-login', 'secondary')),
      intent: 'Group code entry, help, consequences, and submission.',
      constraint: 'Fields are read-only in this gallery. Never enter real credentials into a mock.'
    },
    {
      id: 'recovery-rejected', group: 'Recovery', label: 'Code rejected', title: 'Recover access',
      intro: 'Noema did not accept that code.',
      body: field('recovery-new-code', 'New recovery code', 'Paste the new code', 'Read the new code from config.yaml before trying again.') + recoveryHelp + recoveryRule + actions(go('Continue', 'recovery-enroll'), go('Back to sign in', 'access-login', 'secondary')),
      intent: 'Keep the error beside the cleared field. Explain why the old code cannot be retried.',
      constraint: 'Every parsed attempt rotates the code. Do not add automatic retries.'
    },
    {
      id: 'recovery-enroll', group: 'Recovery', label: 'Create replacement passkey', title: 'Create a new passkey', step: 'Recovery code accepted',
      intro: 'Save a new passkey to finish.',
      body: actions(go('Create passkey', 'recovery-complete')),
      intent: 'Show progress after code acceptance. Keep a clear retry action if enrollment is cancelled.',
      constraint: 'Production starts enrollment immediately. This screen remains available after a cancelled prompt; the setup session lasts five minutes.'
    },
    {
      id: 'recovery-expired', group: 'Recovery', label: 'Recovery session expired', title: 'Start recovery again',
      intro: 'Your recovery session has expired.',
      body: note('Read the current recovery code before you continue.') + actions(go('Enter new code', 'recovery-code'), go('Back to sign in', 'access-login', 'secondary')),
      intent: 'Give a direct restart path without suggesting the original code still works.',
      constraint: 'Existing passkeys remain unchanged.'
    },
    {
      id: 'recovery-complete', group: 'Recovery', label: 'Access restored', title: 'Your access is restored',
      intro: 'Your new passkey is ready.',
      body: status('New passkey saved', '', 'success') + note('You can review your other passkeys and connected apps in Settings.') + actions(finish('Continue to Noema')),
      intent: 'Give brief confirmation and preserve the user’s return destination.',
      constraint: 'Recovery adds access. It does not remove old passkeys or revoke native clients.'
    },
    {
      id: 'provider-choice', group: 'Model setup', label: 'Choose provider', title: 'Choose how Noema thinks', step: 'Set up Noema · 1 of 2',
      intro: 'Choose a provider to get started.',
      body: `<section class="options" aria-label="Model providers">${option('Local', 'Download a model to your Noema server.', 'local-ready', 'Local')}${option('OpenRouter', 'Connect your OpenRouter account.', 'provider-openrouter', 'OpenRouter')}${option('Codex', 'Continue with your Codex sign-in.', 'provider-codex', 'Codex')}</section>` + note('You can add more providers later in Settings.'),
      intent: 'Use ListCardButton from components/ListCardLink.tsx. Match the Task card frame and text metrics, with wrapping descriptions and one action per card.',
      constraint: 'Account and payment requirements must come from verified provider information. Do not invent a universal recommendation.'
    },
    {
      id: 'provider-openrouter', group: 'Model setup', label: 'Connect OpenRouter', title: 'Connect OpenRouter', step: 'Set up Noema · 1 of 2',
      intro: 'Sign in to choose your models.',
      body: actions(go('Continue to OpenRouter', 'provider-waiting'), go('Choose another provider', 'provider-choice', 'secondary')) + note('OpenRouter opens in another tab. Return here after signing in.') + `<details><summary>Use an API key instead</summary>${field('provider-api-key', 'OpenRouter API key', 'Paste your API key')}${actions(go('Connect with API key', 'provider-models'))}</details>`,
      intent: 'Make browser sign-in primary. Keep API-key entry under disclosure.',
      constraint: 'The preview uses no external links and accepts no keys.'
    },
    {
      id: 'provider-codex', group: 'Model setup', label: 'Codex device sign-in', title: 'Sign in to Codex', step: 'Set up Noema · 1 of 2',
      intro: 'Enter this code on the sign-in page.',
      body: `<section class="device-code"><code id="device-example">DEMO-CODE</code>${button('Copy code', 'outline', 'data-copy')}</section>` + actions(go('Open Codex sign-in', 'provider-waiting', 'primary', 'Codex'), go('Cancel connection', 'provider-choice', 'secondary')) + note('Noema continues automatically after you finish signing in.'),
      intent: 'Keep the code beside its copy action. Name the external destination.',
      constraint: 'DEMO-CODE is an inert example. Production displays only the active device attempt’s code.'
    },
    {
      id: 'provider-waiting', group: 'Model setup', label: 'Waiting for sign-in', title: 'Finish signing in', step: 'Set up Noema · 1 of 2',
      intro: 'Finish sign-in in the other tab.',
      body: waiting('Noema continues after you sign in.') + actions(finish('Open sign-in page again', 'outline'), go('Cancel connection', 'provider-choice', 'secondary')),
      intent: 'Keep the waiting state calm and explain which tab owns the next action.',
      constraint: 'Use the exact attempt event and foreground recovery query. Do not introduce polling or infer success from elapsed time.'
    },
    {
      id: 'provider-expired', group: 'Model setup', label: 'Sign-in expired', title: 'Start sign-in again',
      intro: 'This sign-in attempt has expired.',
      body: actions(go('Start sign-in again', 'provider-openrouter'), go('Choose another provider', 'provider-choice', 'secondary')),
      intent: 'Provide a fresh attempt while keeping provider selection available.',
      constraint: 'Production restarts the selected provider’s flow. This example uses OpenRouter.'
    },
    {
      id: 'provider-models', group: 'Model setup', label: 'Review models', title: 'Ready for your first chat', step: 'Set up Noema · 2 of 2',
      intro: 'Change your models later in Settings.', body: '',
      intent: 'Name each selected model once and show its jobs. Keep all nine assignments under customization; update the summary when choices change.',
      constraint: 'Selections remain drafts until explicit confirmation saves the complete setup. Local action reviews remain human approval.'
    },
    {
      id: 'provider-complete', group: 'Model setup', label: 'Setup complete', title: 'Noema is ready',
      intro: 'Settings saved. Opening Chat…',
      body: status('Setup complete', '', 'success') + actions(finish('Open Chat')),
      intent: 'Use a brief completion state without adding a required extra step.',
      constraint: 'Production continues to Chat automatically. The button is a return fallback, not a second confirmation.'
    },
    {
      id: 'local-ready', group: 'Local models', label: 'Before download', title: 'Set up a local model', step: 'Set up Noema · 1 of 2',
      intro: 'Download this model to get started.',
      body: localFacts + localDetails + actions(go('Download model · 5.3 GB', 'local-downloading', 'primary', 'Local'), go('Choose another provider', 'provider-choice', 'secondary')),
      intent: 'Name the model before download. Keep size and computation location visible; disclose license and exact build.',
      constraint: 'This catalog example requires a compatible Metal server. Production must show the selected model’s actual name, size, license, and build.'
    },
    {
      id: 'local-downloading', group: 'Local models', label: 'Download in progress', title: 'Downloading your model',
      intro: 'Downloading to your Noema server.',
      body: `<section class="progress"><progress aria-label="Model download" value="47" max="100">47%</progress>${facts([['2.5 GB of 5.3 GB', '47%']])}</section>` + note('Noema will check the model before you continue.') + actions(go('Cancel download', 'local-ready', 'secondary')),
      intent: 'Keep progress, transferred bytes, and cancellation in one group.',
      constraint: 'Progress is a fixed example. Production uses transfer state. Do not show an invented time estimate.'
    },
    {
      id: 'local-verifying', group: 'Local models', label: 'Checking download', title: 'Checking your model',
      intro: 'The download is complete.',
      body: waiting('Checking the downloaded file') + actions(go('Cancel setup', 'local-ready', 'secondary')),
      intent: 'Explain the work after download completion. Keep it distinct from a stalled transfer.',
      constraint: 'Use this state only while the server reports verification.'
    },
    {
      id: 'local-installed', group: 'Local models', label: 'Model installed', title: 'Your local model is ready',
      intro: 'Review your settings to finish setup.',
      body: status('Model installed', 'Runs on your Noema server', 'success') + actions(go('Review model settings', 'provider-models', 'primary', 'Local')),
      intent: 'Connect installation completion to the final setup review.',
      constraint: 'Do not announce readiness before installation and verification succeed.'
    },
    {
      id: 'local-failed', group: 'Local models', label: 'Download failed', title: 'The download could not finish',
      intro: 'Check the server’s connection.',
      body: actions(go('Try download again', 'local-downloading', 'primary', 'Local'), go('Choose another provider', 'provider-choice', 'secondary')) + `<details><summary>Technical details</summary><p>Example: the download connection was interrupted.</p></details>`,
      intent: 'Keep the recovery action visible. Put exact diagnostics under disclosure.',
      constraint: 'Use the existing transfer retry behavior. Promise resume only when the current transfer supports it.'
    },
    {
      id: 'local-unavailable', group: 'Local models', label: 'No compatible model', title: 'Choose another model provider',
      intro: 'No recommended model fits this server.',
      body: actions(go('Choose another provider', 'provider-choice')) + `<details><summary>Why is Local unavailable?</summary><p>This server does not have enough available memory for the recommended models.</p></details>`,
      intent: 'Give an exit from an unavailable setup path.',
      constraint: 'Do not offer unsupported downloads or describe a phone as the Noema server.'
    },
    {
      id: 'oauth-consent', group: 'Native app connection', label: 'Approve connection', title: 'Connect Noema Desktop?',
      intro: 'Only connect if you started this request.',
      body: `<section class="details"><h2>Complete Noema access</h2><p>Noema Desktop can access all data and actions available through Noema.</p><p>You can revoke this app’s access in Settings.</p></section><section class="decision-actions">${go('Deny', 'oauth-denied', 'outline')}${go('Connect app', 'oauth-connected')}</section>`,
      intent: 'Keep app identity, access consequences, and the decision together.',
      constraint: 'Keep full-access disclosure visible. Preserve recent passkey verification, CSRF, PKCE, and validated return destinations.'
    },
    {
      id: 'oauth-connected', group: 'Native app connection', label: 'Connection complete', title: 'Noema Desktop is connected',
      intro: 'Return to the app to continue.',
      body: status('Connection complete', '', 'success') + actions(finish('Return to Noema Desktop')),
      intent: 'Name the app and offer a clear handoff.',
      constraint: 'Use a return action only when the platform supports it. Otherwise instruct the user to return to the app.'
    },
    {
      id: 'oauth-denied', group: 'Native app connection', label: 'Connection denied', title: 'No new access was granted',
      intro: 'This request did not connect the app.',
      body: note('You can close this tab and return to the app.') + actions(finish('Return to Noema Desktop', 'outline')),
      intent: 'Confirm the user’s decision without presenting denial as a failure.',
      constraint: 'Denying this request does not imply that earlier app grants were revoked.'
    },
    {
      id: 'oauth-expired', group: 'Native app connection', label: 'Request expired', title: 'This connection link has expired',
      intro: 'Start again from the Noema app.',
      body: note('This attempt did not grant new access.') + actions(finish('Return to Noema Desktop')),
      intent: 'Replace protocol language with the state and next action.',
      constraint: 'Show expiration only when the server identifies an expired request.'
    },
    {
      id: 'oauth-invalid', group: 'Native app connection', label: 'Invalid request', title: 'This connection could not start',
      intro: 'The request is invalid or incomplete.',
      body: note('Close this tab. Start the connection again from the Noema app.') + note('This attempt did not grant new access.'),
      intent: 'Provide a complete branded state for malformed authorization requests.',
      constraint: 'Do not use an unvalidated return address or expose callback credentials.'
    },
    {
      id: 'service-connected', group: 'Service callback results', label: 'Sign-in complete', title: 'Google sign-in is complete',
      intro: 'Review tools and permissions in Noema.',
      body: status('Account connected', '', 'success') + actions(finish('Return to Noema')),
      intent: 'This is a browser callback result. Return to its originating Chat card or Settings dialog to finish permissions.',
      constraint: 'Keep connection policy separate from provider authorization. Use only a validated same-origin destination.'
    },
    {
      id: 'service-partial', group: 'Service callback results', label: 'Sign-in complete, tools unavailable', title: 'Signed in, but tools are not ready',
      intro: 'Noema could not load the tools.',
      body: actions(finish('Review connection')) + note('You can retry from the connection settings.'),
      intent: 'Distinguish successful authentication from incomplete connection activation.',
      constraint: 'Do not label this state Connected or discard a valid grant solely because tool listing failed.'
    },
    {
      id: 'service-failed', group: 'Service callback results', label: 'Connection failed', title: 'This connection could not finish',
      intro: 'Review the connection in Noema.',
      body: actions(finish('Review connection')) + `<details><summary>Technical details</summary><p>The provider did not accept this connection request.</p></details>`,
      intent: 'Use the same branded result page for provider and integration callback failures.',
      constraint: 'Do not assert that no access was granted unless the saved attempt proves it.'
    },
    {
      id: 'system-unavailable', group: 'Unavailable states', label: 'Server unavailable', title: 'Noema is unavailable',
      intro: 'Check your connection, then try again.',
      body: actions(go('Try again', 'access-login')) + `<details><summary>If Noema still does not open</summary><p>Ask the person who manages this Noema server to check that it is running.</p></details>`,
      intent: 'Explain an unavailable server without presenting it as a credential failure.',
      constraint: 'An authenticated PWA can retain its governed offline snapshot. Do not replace its offline behavior with this screen.'
    },
    {
      id: 'system-load-error', group: 'Unavailable states', label: 'App failed to load', title: 'Noema could not load',
      intro: 'Reload Noema to try again.',
      body: actions(finish('Reload Noema')),
      intent: 'Use the shared visual treatment for a fatal startup failure.',
      constraint: 'Keep route and content errors inside their existing boundaries. Do not replace the whole app for a local error.'
    }
  ];
  const screenRoot = document.getElementById('screen');
  const flowSelect = document.getElementById('flow');
  const stateSelect = document.getElementById('state');
  const preview = document.getElementById('preview');
  const reviewStatus = document.getElementById('review-status');
  let provider = 'OpenRouter';
  let moveFocus = false;
  const groups = [...new Set(screens.map(screen => screen.group))];
  groups.forEach(group => flowSelect.add(new Option(group, group)));
  const assignmentLabels = ['Chat', 'Simple tasks', 'Medium tasks', 'Difficult tasks', 'Task reviewer', 'Web summaries', 'Progress checks', 'Action reviews', 'Memory updates'];
  function modelSummary(values) {
    const jobGroups = [[[0], 'Chat'], [[1, 2, 4], 'Routine tasks'], [[3], 'Difficult tasks'], [[5, 6, 7, 8], 'Supporting work']];
    return [...new Set(values)].map(model => {
      const jobs = jobGroups.flatMap(([indices, label]) => indices.every(index => values[index] === model)
        ? [label] : indices.filter(index => values[index] === model).map(index => assignmentLabels[index]));
      return `<section class="model-summary-row"><strong>${model}</strong><p>${jobs.join(' · ')}</p></section>`;
    }).join('');
  }
  function modelBody() {
    // Example choices from the source baseline, not a live provider catalog.
    const choices = provider === 'Local' ? ['Gemma 4 E4B IT'] : ['GPT-5.6 Luna', 'GPT-5.6 Sol', 'GPT-5.6 Terra'];
    const values = assignmentLabels.map((_, index) => provider === 'Local' ? index === 7 ? 'Ask me for approval' : choices[0]
      : index === 3 ? choices[1] : provider === 'Codex' && index === 0 ? choices[2] : choices[0]);
    const assignments = assignmentLabels.map((label, index) => provider === 'Local' && index === 7
      ? `<section class="assignment"><strong>Action reviews</strong><p>Ask me for approval</p></section>`
      : `<label class="assignment" for="model-${index}">${label}<select id="model-${index}">${choices.map(model => `<option${model === values[index] ? ' selected' : ''}>${model}</option>`).join('')}</select></label>`).join('');
    return status(`${provider} connected`, '', 'success') + `<section id="model-summary" class="model-summary" aria-label="Selected models">${modelSummary(values)}</section><details><summary>Customize models</summary>${assignments}</details>` + actions(go('Confirm and start Chat', 'provider-complete'), go('Choose another provider', 'provider-choice', 'secondary'));
  }
  function render() {
    const id = location.hash.slice(1);
    const screen = screens.find(item => item.id === id) || screens[0];
    flowSelect.value = screen.group;
    stateSelect.replaceChildren(...screens.filter(item => item.group === screen.group).map(item => new Option(item.label, item.id)));
    stateSelect.value = screen.id;
    const body = screen.id === 'provider-models' ? modelBody() : screen.body;
    screenRoot.innerHTML = `<header class="screen-heading">${screen.step ? `<p class="step">${screen.step}</p>` : ''}<h1 id="screen-title" tabindex="-1">${screen.title}</h1><p class="intro">${screen.intro}</p></header><section class="screen-body">${body}</section>`;
    document.getElementById('intent').textContent = screen.intent;
    document.getElementById('constraint').textContent = screen.constraint;
    document.title = `${screen.label} · Noema UI plan`;
    reviewStatus.textContent = '';
    if (moveFocus) {
      const target = screen.id === 'access-login' ? screenRoot.querySelector('.primary')
        : screen.id === 'recovery-code' || screen.id === 'recovery-rejected' ? screenRoot.querySelector('input')
        : document.getElementById('screen-title');
      target.focus({ preventScroll: true });
    }
    moveFocus = false;
  }
  function navigate(id, focus = false) {
    moveFocus = focus;
    if (location.hash !== `#${id}`) history.pushState(null, '', `#${id}`);
    render();
  }
  flowSelect.addEventListener('change', () => navigate(screens.find(screen => screen.group === flowSelect.value).id));
  stateSelect.addEventListener('change', () => navigate(stateSelect.value));
  document.getElementById('width').addEventListener('change', event => { preview.dataset.width = event.target.value; });
  screenRoot.addEventListener('click', async event => {
    const button = event.target.closest('button');
    if (!button) return;
    if (button.dataset.provider) provider = button.dataset.provider;
    if (button.dataset.go) navigate(button.dataset.go, true);
    if (button.hasAttribute('data-finish')) reviewStatus.textContent = 'Preview complete. The application is unchanged.';
    if (button.hasAttribute('data-copy')) {
      try {
        await navigator.clipboard.writeText('DEMO-CODE');
        reviewStatus.textContent = 'Example code copied.';
      } catch {
        const selection = window.getSelection();
        const range = document.createRange();
        range.selectNodeContents(document.getElementById('device-example'));
        selection.removeAllRanges(); selection.addRange(range);
        reviewStatus.textContent = 'Example code selected. Use your browser’s Copy command.';
      }
    }
  });
  screenRoot.addEventListener('change', event => {
    if (event.target.matches('.assignment select')) {
      const values = assignmentLabels.map((_, index) => document.getElementById(`model-${index}`)?.value || 'Ask me for approval');
      document.getElementById('model-summary').innerHTML = modelSummary(values);
    }
  });
  window.addEventListener('hashchange', render);
  render();
})();
