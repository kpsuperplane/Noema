/* Live component gallery for the Noema Design System spec page.
   Self-contained: inlines a Lucide-backed Icon and mounts every primitive. */

const DS = window.NoemaDesignSystem_3d237e;
const { Button, IconButton, Input, Textarea, Select, Checkbox, Switch,
        Card, Badge, Avatar, Tag, Tabs, Dialog, Toast, Tooltip,
        ChatMessage, ToolCall } = DS;

function Icon({ name, size = 18, strokeWidth, style }) {
  const node = window.lucide && window.lucide.icons && window.lucide.icons[name];
  if (!node) return null;
  const base = node[1] || {}, kids = node[2] || [];
  return React.createElement('svg', {
    width: size, height: size, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor',
    strokeWidth: strokeWidth || base['stroke-width'] || 2, strokeLinecap: 'round', strokeLinejoin: 'round', style,
    'aria-hidden': 'true',
  }, kids.map((c, i) => React.createElement(c[0], { key: i, ...c[1] })));
}

const Plus = () => <Icon name="Plus" size={15} />;
const Gear = () => <Icon name="Settings" size={17} />;

function Block({ label, children, span }) {
  return (
    <div className="gx-block" style={span ? { gridColumn: '1 / -1' } : null}>
      <div className="gx-block__label">{label}</div>
      <div className="gx-block__body">{children}</div>
    </div>
  );
}

function Gallery() {
  const [tab, setTab] = React.useState('chat');
  const [dialogOpen, setDialogOpen] = React.useState(false);

  React.useEffect(() => { if (window.lucide) window.lucide.createIcons(); });

  return (
    <div className="gx-grid">
      <Block label="Button — variants">
        <div className="gx-row">
          <Button variant="primary">Primary</Button>
          <Button variant="accent">Accent</Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="danger">Danger</Button>
        </div>
      </Block>

      <Block label="Button — sizes, icons & states">
        <div className="gx-row">
          <Button size="sm">Small</Button>
          <Button leftIcon={<Plus />}>New agent</Button>
          <Button size="lg" variant="accent">Get started</Button>
          <Button loading>Working</Button>
          <Button disabled>Disabled</Button>
        </div>
      </Block>

      <Block label="IconButton">
        <div className="gx-row">
          <IconButton aria-label="Settings" icon={<Gear />} />
          <IconButton aria-label="Settings" variant="soft" icon={<Gear />} />
          <IconButton aria-label="Settings" variant="outline" icon={<Gear />} />
          <IconButton aria-label="Add" variant="primary" icon={<Plus />} />
          <IconButton aria-label="Add" variant="primary" round icon={<Plus />} />
        </div>
      </Block>

      <Block label="Badge & Tag">
        <div className="gx-row" style={{ marginBottom: 12 }}>
          <Badge tone="success" dot>Online</Badge>
          <Badge tone="brand">Open source</Badge>
          <Badge tone="warning">Needs review</Badge>
          <Badge tone="danger">Failed</Badge>
          <Badge tone="info">Beta</Badge>
          <Badge tone="neutral" mono>v0.4.2</Badge>
        </div>
        <div className="gx-row">
          <Tag tone="brand" mono icon={<Icon name="Hash" size={11} />}>filesystem</Tag>
          <Tag selectable selected>web-search</Tag>
          <Tag onRemove={() => {}}>shell</Tag>
        </div>
      </Block>

      <Block label="Avatar — agents vs. people">
        <div className="gx-row" style={{ alignItems: 'center' }}>
          <Avatar name="Ada Lovelace" status />
          <Avatar name="Kit Mercer" size="sm" />
          <Avatar kind="agent" square name="Research Agent" />
          <Avatar kind="agent" square size="lg" name="WB" ring />
        </div>
      </Block>

      <Block label="Tabs">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <Tabs variant="underline" items={[
            { value: 'chat', label: 'Chat' }, { value: 'tools', label: 'Tools', count: 4 }, { value: 'memory', label: 'Memory' },
          ]} value={tab} onChange={setTab} />
          <Tabs variant="segmented" items={[
            { value: 'chat', label: 'Chat' }, { value: 'tools', label: 'Tools' }, { value: 'memory', label: 'Memory' },
          ]} value={tab} onChange={setTab} />
        </div>
      </Block>

      <Block label="Inputs">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <Input label="Workspace name" defaultValue="research-agent" />
          <Input label="Search" size="sm" leftIcon={<Icon name="Search" size={15} />} placeholder="Filter tools…" />
          <Select label="Model" options={['Claude Sonnet', 'Llama 3 70B', 'Mistral Large']} />
        </div>
      </Block>

      <Block label="Prompt, toggles & choices">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <Textarea label="System prompt" mono rows={3} defaultValue={'You are a careful research assistant.'} />
          <Checkbox label="Run trusted tools automatically" defaultChecked />
          <Switch label="Local-only mode" defaultChecked />
        </div>
      </Block>

      <Block label="Cards">
        <div className="gx-cards">
          <Card><div className="gx-ct">Default</div><div className="gx-cb">White surface, hairline border, soft warm shadow.</div></Card>
          <Card variant="brand"><div className="gx-ct">Brand</div><div className="gx-cb">Pine-tinted highlight surface.</div></Card>
          <Card variant="sunken"><div className="gx-ct">Sunken</div><div className="gx-cb">Recessed panel for nested content.</div></Card>
        </div>
      </Block>

      <Block label="Feedback — Dialog · Toast · Tooltip">
        <div className="gx-row" style={{ marginBottom: 14 }}>
          <Button variant="secondary" onClick={() => setDialogOpen(true)}>Open dialog</Button>
          <Tooltip label="New conversation" kbd="⌘N"><Button variant="ghost">Hover for tooltip</Button></Tooltip>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10, maxWidth: 360 }}>
          <Toast tone="success" title="Agent deployed" message="research-agent is now live." onClose={() => {}} />
          <Toast tone="warning" title="Tool needs approval" message="shell.exec is awaiting your OK." />
        </div>
        {dialogOpen && (
          <Dialog open title="Grant filesystem access?"
            description="Research Agent is requesting read access to ~/Documents."
            onClose={() => setDialogOpen(false)}
            footer={<>
              <Button variant="ghost" onClick={() => setDialogOpen(false)}>Not now</Button>
              <Button onClick={() => setDialogOpen(false)}>Allow</Button>
            </>}>
            Tools can be revoked anytime from Settings → Permissions.
          </Dialog>
        )}
      </Block>

      <Block label="Agent — ChatMessage & ToolCall" span>
        <div style={{ maxWidth: 620 }}>
          <ChatMessage role="user" name="You" time="2:14 PM" avatar={<Avatar name="You" size="sm" />}>
            Find the most-cited paper on retrieval-augmented generation and summarize it.
          </ChatMessage>
          <div style={{ margin: '2px 0 6px 36px', display: 'flex', flexDirection: 'column', gap: 8 }}>
            <ToolCall tool="web_search" status="success" args={{ query: 'most cited RAG paper' }} result="Lewis et al. 2020" />
            <ToolCall tool="fetch_url" status="running" />
          </div>
          <ChatMessage role="agent" name="Research Agent" time="2:14 PM" avatar={<Avatar kind="agent" square size="sm" name="RA" />}>
            <p style={{ margin: 0 }}>The most-cited work is <b>Lewis et&nbsp;al. (2020)</b> — it pairs a retriever with a generator so answers stay grounded in sources you can check.</p>
          </ChatMessage>
          <ChatMessage role="agent" typing avatar={<Avatar kind="agent" square size="sm" name="RA" />} />
        </div>
      </Block>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('gallery-root')).render(<Gallery />);
