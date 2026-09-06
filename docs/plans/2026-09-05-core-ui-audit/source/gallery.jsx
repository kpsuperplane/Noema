import React, {useState, useEffect, useRef} from 'react';
import {createRoot} from 'react-dom/client';
import {Button} from '@astryxdesign/core/Button';
import {IconButton} from '@astryxdesign/core/IconButton';
import {HStack} from '@astryxdesign/core/HStack';
import {VStack} from '@astryxdesign/core/VStack';
import {Selector} from '@astryxdesign/core/Selector';
import {TextInput} from '@astryxdesign/core/TextInput';
import {DropdownMenu} from '@astryxdesign/core/DropdownMenu';
import {Switch} from '@astryxdesign/core/Switch';
import {Tab, TabList} from '@astryxdesign/core/TabList';
import {MarkdownContent} from '@/components/MarkdownContent';
import {Dialog as ResponsiveDialog} from '@/components/ResponsiveDialog';
import {DialogHeader} from '@astryxdesign/core/Dialog';
import {Layout, LayoutContent, LayoutFooter} from '@astryxdesign/core/Layout';
import {StatusDot} from '@astryxdesign/core/StatusDot';
import {Inbox, Repeat2, CalendarClock, Play, Pause, Square, X, Plus, Check, Ellipsis, KeyRound} from 'lucide-react';
import {TaskStatusBadge} from '@/components/chatDetail/task/TaskStatusBadge';
import {ListCardButton} from '@/components/ListCardLink';
import {MarkdownInlineEditor} from '@/components/MarkdownEditor';
import {SettingsSection, SettingsList, SettingsListItem, SettingsSectionInset} from '@/components/settings/SettingsPrimitives';
import {ApolloClient, ApolloLink, InMemoryCache, Observable} from '@apollo/client';
import {ApolloProvider} from '@apollo/client/react';
import {createRootRoute, createRoute, createRouter, createMemoryHistory, RouterProvider} from '@tanstack/react-router';
import {AppShell} from '@/components/shell/AppShell';
import {ShellPageLayout, ShellPageTrack} from '@/components/shell/ShellPageLayout';
import {ShellSectionHeader} from '@/components/shell/ShellSectionHeader';
import {MasterDetailLayout} from '@/components/shell/MasterDetailLayout';
import {SettingsManagementLayout} from '@/components/settings/SettingsManagementLayout';
import {TasksToolbar} from '@/components/tasks/TasksToolbar';
import {MotionRoot} from '@/motion/MotionRoot';
import {hrefForRoute} from '@/app/routes';
import '@/styles.css';
import './gallery.css';

const studies = [
 ['inbox','Inbox task','01','Keep task instructions and next steps clear.','The current detail uses “No agent run yet” and “No output yet.” The mock gives an unstarted task a useful summary.'],
 ['recurring','Recurring task','02','Make the next run clear. Make past runs useful.','Current rows repeat “Open task.” Outcome labels below need linked task status; they must not be inferred from occurrence creation.'],
 ['schedule','Schedule dialog','03','Edit a routine without learning cron.','The current editor initializes every recurrence as Custom cron. Preserve matching presets and show one compact schedule preview.'],
 ['providers','Provider settings','04','Put connection health and useful facts first.','Current details separate Authentication, Status, and Default account into tall rows. Capability IDs and contract fields remain prominent.'],
 ['notifications','Notification settings','05','Start with this device. Keep server setup available.','Apple push configuration currently appears before device notifications. The mock keeps both functions on the same page.'],
 ['task-settings','Task options','06','Keep task options beside the task title.','Project and task agent are direct choices in the info row. Recurring schedule settings stay above run history.'],
 ['start','Start task dialog','07','Name the task and explain the next step.','Copy reference for a required confirmation. The existing floating bar starts tasks directly; this proposal adds no confirmation step.'],
 ['cancel','Cancel task dialog','08','Make the two choices impossible to confuse.','The current dialog can present a dismiss button labeled “Cancel” beside a destructive task action also labeled “Cancel.”'],
 ['agents','Agent and task models','10','Let people read the model they selected.','The rendered Agents page truncates model names while Reasoning and Fast controls stay visible. Keep related model groups together.'],
 ['capture','New task','09','Make saving to Inbox easy to discover.','The current capture puts “Add to Inbox” inside a split-button menu. Project, scheduling, and developer controls are all icon-only.']
];
const samples=[['Plan a weekend in Portland','Personal · Added just now'],['Compare home internet plans','Home · Added yesterday'],['Find a beginner pottery class','Personal · Added yesterday']];
studies.sort((a,b)=>Number(a[2])-Number(b[2]));
const smallIcon=(C)=><C size={16} aria-hidden="true"/>;
const B=({children,...props})=><Button type="button" size="md" variant="primary" label={children} {...props}/>;
function Status({children,variant='success'}) {return <HStack as="span" align="center" gap={1}><StatusDot variant={variant} label={children}/><span className="small">{children}</span></HStack>}
function useStudy(){
 const read=()=>studies.find(s=>s[0]===location.hash.slice(1))?.[0]||'inbox';
 const [id,setId]=useState(read);
 useEffect(()=>{const change=()=>setId(read());window.addEventListener('hashchange',change);return()=>window.removeEventListener('hashchange',change)},[]);
 return id;
}
function App(){
 const id=useStudy(),frame=useRef(null);
 const [phone,setPhone]=useState(false),[phase,setPhase]=useState('inbox');
 useEffect(()=>{const change=e=>{if(e.origin!==location.origin||e.source!==frame.current?.contentWindow)return;if(e.data?.type==='mock-study'&&studies.some(s=>s[0]===e.data.id))location.hash=e.data.id;if(e.data?.type==='mock-task-phase'&&['inbox','queued','running'].includes(e.data.phase))setPhase(e.data.phase)};window.addEventListener('message',change);return()=>window.removeEventListener('message',change)},[]);
 const study=studies.find(s=>s[0]===id)||studies[0];
 return <VStack className="gallery" gap={4}>
  <HStack as="header" justify="between" align="center" wrap="wrap" gap={3} className="gallery-header">
   <VStack gap={0.5}><strong className="gallery-brand">Noema <span> / Core UI study</span></strong><p className="small muted">Proposed refinements · Sample data · UI unchanged</p></VStack>
   <B variant="secondary" aria-pressed={phone} onClick={()=>setPhone(!phone)}>{phone?'Desktop width':'Phone width'}</B>
  </HStack>
  <nav className="study-nav" aria-label="Mock gallery">{studies.map(s=><a key={s[0]} href={'#'+s[0]} aria-current={s[0]===id?'page':undefined}><span>{s[2]}</span>{s[1]}</a>)}</nav>
  <HStack justify="between" align="center" gap={3} className="study-heading"><h1>{study[3]}</h1><span className="small muted">{study[2]} / 10</span></HStack>
  {['inbox','task-settings'].includes(id)?<HStack gap={3} align="center" wrap="wrap"><Selector label="Preview task state" size="sm" value={phase} options={[{value:'inbox',label:'Inbox'},{value:'queued',label:'Queued'},{value:'running',label:'Running'}]} onChange={phase=>{setPhase(phase);frame.current?.contentWindow.postMessage({type:'mock-task-phase',phase},location.origin)}}/><p className="small muted">Compare the same task as it starts. This control belongs to the gallery.</p></HStack>:null}
  <section className={'preview '+(phone?'phone':'')} aria-label={study[1]+' mock'} key={id+phone}>
   <iframe ref={frame} title={study[1]+' in the Noema shell'} src={'?preview#'+id}/>
  </section>
  <aside className="annotation"><strong>Why this change</strong><p>{study[4]}</p><p className="small muted">Based on source and read-only app inspection. The preview uses the actual app shell and sample data. Other app destinations are outside this study.</p></aside>
 </VStack>
}
const settingsSections={agents:'agents',providers:'system-providers',notifications:'system-notifications'};
const routeForStudy=id=>settingsSections[id]?{kind:'settings',section:settingsSections[id]}:{kind:'tasks'};
function Preview(){
 const id=useStudy();
 useEffect(()=>{const path=hrefForRoute(routeForStudy(id));if(previewRouter.state.location.pathname!==path)void previewRouter.navigate({to:path});if(parent!==window)parent.postMessage({type:'mock-study',id},location.origin)},[id]);
 return <Scene key={id} id={id}/>;
}
function Scene({id}){
 const isDialog=['schedule','start','cancel'].includes(id),recurring=['recurring','schedule'].includes(id);
 const [modal,setModal]=useState(isDialog?id:null),[notice,setNotice]=useState(''),[paused,setPaused]=useState(false),[selected,setSelected]=useState(0),[phase,setPhase]=useState(id==='cancel'?'running':'inbox'),[cancelled,setCancelled]=useState(false),[detail,setDetail]=useState(!isDialog);
 const [savedTitle,setSavedTitle]=useState('');
 useEffect(()=>{const change=e=>{if(e.origin===location.origin&&e.source===parent&&e.data?.type==='mock-task-phase'&&['inbox','queued','running'].includes(e.data.phase))setPhase(e.data.phase)};window.addEventListener('message',change);return()=>window.removeEventListener('message',change)},[]);
 useEffect(()=>{if(parent!==window)parent.postMessage({type:'mock-task-phase',phase},location.origin)},[phase]);
 const done=message=>{setNotice(message);setModal(null)};
 const open=type=>setModal(type);
 const isSettings=Boolean(settingsSections[id]);
 const navigate=async route=>{
  const target=route.kind==='tasks'?'inbox':route.kind==='settings'?Object.keys(settingsSections).find(id=>settingsSections[id]===route.section):null;
  if(target)location.hash=target;
  else setNotice('This destination is outside the ten layout studies.');
 };
 const taskContent=<section className="task-detail">
  {id==='capture'?<HStack as="header" className="detail-heading" justify="between" align="center"><strong>New task</strong><Button isIconOnly size="sm" variant="ghost" label="Close task details" icon={smallIcon(X)} onClick={()=>setDetail(false)}/></HStack>:null}
  <section className="task-detail-scroll">
   {recurring?<Recurrence key={selected} onClose={()=>setDetail(false)} notice={setNotice} title={selected?'Weekly meal ideas':'Morning reading'} paused={paused} toggle={()=>{setPaused(!paused);setNotice(paused?'Schedule resumed in this mock.':'Schedule paused in this mock.')}} open={open}/>:id==='capture'?<Capture onDone={m=>{setSavedTitle(m);done('Task saved in this mock.')}}/>:<InboxDetail key={selected} onClose={()=>setDetail(false)} selected={selected} phase={phase} cancelled={cancelled} open={open} onStart={()=>setPhase('queued')}/>}
  </section>
 </section>;
 return <AppShell route={routeForStudy(id)} status={{primaryAgentDisplayName:'Momo'}} socketState="open" recovery={{state:'ready',updating:false}} onNavigate={navigate}>
  <section className="mock-page"><ShellPageLayout width={isSettings&&id!=='providers'?'centered':'fluid'}>
   {id==='providers'?<Provider open={open}/>:isSettings?<section className="settings-page">
    <ShellSectionHeader title={id==='agents'?'Agents':'Notifications'}/><ShellPageTrack>{id==='agents'?<Agents notice={setNotice}/>:<Notifications notice={setNotice}/>}</ShellPageTrack>
   </section>:<MasterDetailLayout detailOpen={detail} detailLabel={id==='capture'?'New task':'Task and artifact details'} onDetailOpenChange={setDetail} list={<section className="task-list-page">
    <TasksToolbar onNewTask={()=>id==='capture'?setDetail(true):location.hash='capture'}/>
    <section className="task-list"><HStack className="task-list-heading" justify="between" align="center"><h3>{recurring?'Scheduled':phase==='inbox'?'Inbox':'In progress'}</h3><span className="small muted">{recurring?'2':phase==='inbox'?'3':'1'}</span></HStack>
     {isDialog?<B className="reopen-dialog" variant="secondary" onClick={()=>open(id)}>Open {id==='schedule'?'schedule':id==='start'?'start task':'cancel task'} dialog</B>:null}
     <VStack gap={2}>{(recurring?[['Morning reading','Weekdays · 8:00 AM'],['Weekly meal ideas','Sundays · 10:00 AM']]:phase==='inbox'?samples:[samples[selected],...samples.filter((_,i)=>i!==selected)]).map((t,index)=>{
      const i=recurring?index:samples.indexOf(t),active=!recurring&&phase!=='inbox'&&i===selected;
      return <React.Fragment key={t[0]}>{!recurring&&phase!=='inbox'&&index===1?<HStack className="remaining-tasks" justify="between"><h3>Inbox</h3><span className="small muted">2</span></HStack>:null}<ListCardButton selected={detail&&id!=='capture'&&selected===i} onClick={()=>{if(i!==selected)setPhase('inbox');setSelected(i);if(id==='capture')location.hash='inbox';else setDetail(true)}}><HStack gap={2} align="start">{smallIcon(recurring?Repeat2:Inbox)}<VStack as="span" gap={1}><strong>{t[0]}</strong><span className="small muted">{active?(phase==='queued'?'Queued':'Running')+' · Personal':t[1]}</span></VStack></HStack></ListCardButton></React.Fragment>;
     })}</VStack>
    </section>
   </section>} detail={detail||isDialog?taskContent:null}/>}
  </ShellPageLayout></section>
  {notice?<p className="mock-notice" role="status"><Check size={14}/>{notice}{savedTitle?' '+savedTitle:''}</p>:null}
  {modal?<MockDialog type={modal} taskTitle={recurring?(selected?'Weekly meal ideas':'Morning reading'):samples[selected][0]} onClose={()=>setModal(null)} onDone={message=>{if(modal==='start')setPhase('queued');if(modal==='cancel')setCancelled(true);done(message)}}/>:null}
 </AppShell>
}
// Capture and existing tasks use the same title field and production Markdown editor.
function TaskTitleField({value,onChange,onCommit,error,focus=false,readOnly=false}){
 return <input readOnly={readOnly} data-autofocus={focus?true:undefined} aria-label="Task title" aria-invalid={error?true:undefined} className="capture-title" placeholder="What needs to be done?" value={value} onChange={e=>onChange(e.target.value)} onBlur={()=>onCommit?.(value)} onKeyDown={e=>{if(e.key==='Enter'&&!e.nativeEvent.isComposing){e.preventDefault();e.currentTarget.blur()}}}/>;
}
function TaskInstructionsField({value,onChange,onCommit,sourceMode,onSourceModeChange,onPendingChange,fill=false}){
 const editor=useRef(null);
 const change=value=>{onPendingChange?.(false);onChange(value);if(!editor.current?.contains(document.activeElement))onCommit?.(value)};
 return <section ref={editor} data-slot={fill?'task-capture-document':undefined} onClick={e=>{if(fill&&!e.target.closest('button,input,textarea,a,[contenteditable="true"]'))editor.current?.querySelector('[contenteditable="true"],textarea')?.focus()}} onInput={()=>{if(!sourceMode)onPendingChange?.(true)}} aria-label="Task instructions" className="task-instructions-editor" onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget))onCommit?.(value)}}>
  <MarkdownInlineEditor value={value} onChange={change} label="Instructions" placeholder="Add details or instructions…" sourceMode={sourceMode} onSourceModeChange={onSourceModeChange}/>
 </section>
}
function TaskDocument({title,metadata,timing,instructions,note,advanced,children,onClose,phase='inbox',showTaskViews=false}){
 const [tab,setTab]=useState('workspace');
 const active=phase!=='inbox';
 const [draft,setDraft]=useState({title,instructions}),[saved,setSaved]=useState({title,instructions}),[feedback,setFeedback]=useState(''),[error,setError]=useState('');
 const change=(field,value)=>{setDraft(current=>({...current,[field]:value}));setFeedback('');if(field==='title')setError('')};
 const commit=(field,value)=>{if(field==='title'&&!value.trim()){setError('Add a task title.');return}if(value!==saved[field]){setSaved(current=>({...current,[field]:value}));setFeedback('Saved in this mock')}};
 return <VStack className="task-document" gap={3}>
  <VStack as="header" className="task-document-header" gap={1}>
   <HStack className="task-title-row" align="center" gap={2}><TaskTitleField readOnly={active} value={draft.title} onChange={value=>change('title',value)} onCommit={value=>commit('title',value)} error={error}/><Button isIconOnly data-autofocus size="sm" variant="ghost" label="Close task details" icon={smallIcon(X)} onClick={onClose}/></HStack>
   {showTaskViews?<TabList size="sm" hasDivider aria-label="Task detail view" value={tab} onChange={setTab}><Tab label="Workspace" value="workspace"/><Tab label="Transcript" value="transcript"/></TabList>:null}
  </VStack>
  {error?<p role="alert" className="small">{error}</p>:null}
  {showTaskViews&&tab==='transcript'?<VStack as="section" aria-label="Transcript" gap={2}><MarkdownContent density="compact">{phase==='inbox'?'Noema’s updates will appear here after you start this task.':phase==='queued'?'Noema will add updates here when the task starts.':'I’m checking options against your instructions. I’ll save the result in the workspace.'}</MarkdownContent></VStack>:<VStack as="section" aria-label="Workspace" gap={3}>
   {typeof metadata==='string'?<p className="small muted">{metadata}</p>:metadata}
   {timing}
   <VStack gap={2}>{active?<MarkdownContent density="compact">{draft.instructions}</MarkdownContent>:<TaskInstructionsField value={draft.instructions} onChange={value=>change('instructions',value)} onCommit={value=>commit('instructions',value)}/>}{note?<p className="small muted">{note}</p>:null}{feedback?<p className="small muted" role="status">{feedback}</p>:null}</VStack>
   {advanced}
   {children}
  </VStack>}
 </VStack>
}
function TaskTiming({label,value,paused=false,onSchedule,scheduled=false}){
 return <HStack as="section" className={'task-timing '+(paused?'paused':'')} align="center" justify="between" gap={2} wrap="wrap">
  <VStack gap={1}><span className="small">{label}</span><strong>{value}</strong></VStack>
  <B variant="ghost" icon={smallIcon(CalendarClock)} onClick={onSchedule}>{scheduled?'Reschedule':'Schedule'}</B>
 </HStack>
}
function TaskDock({status,children}){
 return <HStack as="aside" aria-label="Task actions" className="task-dock" align="center" justify="between" gap={2}><strong className="small">{status}</strong><HStack gap={1} align="center" role="group" aria-label="Task controls">{children}</HStack></HStack>
}
function TaskInfo({selected,readOnly=false}){
 const [project,setProject]=useState(selected===1?'home':'personal'),[agent,setAgent]=useState('built-in'),[saved,setSaved]=useState('');
 return <HStack className="task-info" gap={2} align="center" wrap="wrap">
  <Selector label="Project" isDisabled={readOnly} isLabelHidden size="sm" value={project} options={[{value:'personal',label:'Personal'},{value:'home',label:'Home'},{value:'none',label:'No project'}]} onChange={v=>{setProject(v);setSaved('Saved in this mock')}}/>
  <Selector label="Task agent" isDisabled={readOnly} isLabelHidden size="sm" value={agent} options={[{value:'built-in',label:'Noema (built-in)'},{value:'local',label:'Local agent (ACP)'}]} onChange={v=>{setAgent(v);setSaved('Saved in this mock')}}/>
  <span role="status" className="small muted">{saved}</span>
 </HStack>
}
function ScheduleSettings(){
 const [saved,setSaved]=useState('');
 return <VStack as="section" className="compact-section task-advanced" gap={2} aria-label="Advanced settings">
  <HStack justify="between" align="center" gap={2}><h3>Advanced settings</h3><span role="status" className="small muted">{saved}</span></HStack>
  <ScheduleBehavior onSaved={()=>setSaved('Saved in this mock')}/>
 </VStack>
}
function ScheduleBehavior({onSaved=()=>{},once=false}){
 const [missed,setMissed]=useState('run_once'),[overlap,setOverlap]=useState('skip');
 return <VStack gap={3}>
  <Selector size="sm" label="If Noema misses a run" value={missed} options={[{value:'run_once',label:'Run once when Noema is available'},{value:'skip',label:'Skip the missed run'}]} onChange={v=>{setMissed(v);onSaved()}}/>
  {!once?<Selector size="sm" label="If the previous run is still active" value={overlap} options={[{value:'skip',label:'Skip the next run'},{value:'queue',label:'Queue one run'},{value:'allow',label:'Allow both runs'}]} onChange={v=>{setOverlap(v);onSaved()}}/>:null}
 </VStack>
}
function InboxDetail({selected,phase,cancelled,open,onStart,onClose}){
 return <>
  <TaskDocument showTaskViews phase={phase} onClose={onClose} title={samples[selected][0]} metadata={<TaskInfo selected={selected} readOnly={phase!=='inbox'}/>}
   timing={phase==='inbox'?<TaskTiming label="Timing" value="No scheduled start" onSchedule={()=>open('task-schedule')}/>:null}
   instructions={selected===0?'Find a relaxed weekend plan for two people. Include places to eat, a walk, and one indoor option.\n\nKeep the total budget under $400. Include links so I can review the options.':selected===1?'Compare monthly costs, speeds, and contract terms. Keep the recommendations short.':'Find beginner classes with evening or weekend sessions. Include prices and what materials are provided.'}/>
  <TaskDock status={cancelled?'Task cancelled':phase==='inbox'?'Ready when you are':phase==='queued'?'Queued':'Running'}>
   {phase==='inbox'?<IconButton className="task-dock-control" size="sm" variant="ghost" icon={<Play size={15} aria-hidden="true" color="var(--noema-pine-700)" fill="var(--noema-pine-700)"/>} label="Start task" tooltip="Start task" isDisabled={cancelled} onClick={onStart}/>:<IconButton className="task-dock-control" size="sm" variant="ghost" icon={smallIcon(Square)} label="Cancel task" tooltip="Cancel task" isDisabled={cancelled} onClick={()=>open('cancel')}/>}
  </TaskDock>
 </>
}
function Recurrence({title,paused,toggle,open,notice,onClose}){
 const [run,setRun]=useState(null);
 return <>
 <TaskDocument onClose={onClose} title={title} metadata={(title==='Morning reading'?'Weekdays at 8:00 AM':'Sundays at 10:00 AM')+' · Pacific time'}
  timing={<TaskTiming label={paused?'Schedule paused':'Next run'} value={paused?'No new scheduled runs':title==='Morning reading'?'Monday, Sep 7 · 8:00 AM':'Sunday, Sep 6 · 10:00 AM'} paused={paused} scheduled onSchedule={()=>open('recurrence-schedule')}/>}
  instructions={title==='Morning reading'?'Find three thoughtful articles about design and technology. Give me a short summary and a link for each.':'Suggest five easy dinners for next week. Include a shopping list and keep preparation under 30 minutes.'}
  note="Edits apply to future runs." advanced={<ScheduleSettings/>}>
  <VStack gap={2}><HStack justify="between" align="center"><h3>Run history</h3><span className="small muted">3 recent runs</span></HStack><VStack gap={1.5} className="history">{[
 ['Fri, Sep 4','done','Done','3 articles ready'],
 ['Thu, Sep 3','waiting_for_human','Needs you','Sign-in needed'],
 ['Wed, Sep 2','failed','Failed','The source could not be reached']
].map((r,i)=><ListCardButton className="history-task-card" key={r[0]} selected={run===i} onClick={()=>setRun(i)}>
 <HStack justify="between" gap={2} align="start"><strong>{title}</strong><span className="small muted">{r[0]}</span></HStack>
 <span className="small muted">{r[3]}</span>
 <HStack gap={1} align="center"><TaskStatusBadge status={r[1]} label={r[2]}/><span className="small muted">· 8:00 AM · Personal</span></HStack>
</ListCardButton>)}</VStack>{run!==null?<p role="status" className="small muted">Sample run selected. In the app, this opens the linked task and its {run===1?'request for help':'saved details'}.</p>:null}</VStack>
 </TaskDocument>
 <TaskDock status={paused?'Paused':'Active'}>
  <IconButton className="task-dock-control" size="sm" variant="ghost" icon={smallIcon(paused?Play:Pause)} label={paused?'Resume schedule':'Pause schedule'} tooltip={paused?'Resume schedule':'Pause schedule'} onClick={toggle}/>
  <IconButton className="task-dock-control" size="sm" variant="ghost" icon={<Play size={15} aria-hidden="true" color="var(--noema-pine-700)" fill="var(--noema-pine-700)"/>} label="Run now" tooltip="Run now" onClick={()=>notice('Task queued in this mock. The regular schedule stays in place.')}/>
  <DropdownMenu button={{label:"Recurring task actions",icon:smallIcon(Ellipsis),isIconOnly:true,variant:"ghost",size:"sm",className:"task-dock-control"}} hasChevron={false} items={[{label:"Skip next run",isDisabled:paused,onClick:()=>notice("The next run would be skipped. This mock keeps its example dates.")},{type:"divider"},{label:"End recurring task",onClick:()=>open("end")}]} />
 </TaskDock>
 </>
}
function Provider({open}){
 const [selected,setSelected]=useState(()=>innerWidth>=980?'OpenAI':null);
 return <SettingsManagementLayout title="Providers" primaryAction={{label:'Add provider',onClick:()=>open('add-provider')}} detailLabel="Provider details" detailOpen={selected!==null} onDetailOpenChange={open=>{if(!open)setSelected(null)}}
  list={<VStack gap={2}>{['OpenAI','Anthropic'].map(n=><ListCardButton key={n} selected={selected===n} onClick={()=>setSelected(n)}><HStack align="center" justify="between"><VStack as="span" gap={1}><strong>{n}</strong><span className="small muted">{n==='OpenAI'?'Personal account':'API key'}</span></VStack><Status>Connected</Status></HStack></ListCardButton>)}</VStack>} detail={selected?<VStack className="settings-content" gap={3}><HStack as="header" className="provider-detail-header" align="center" justify="between"><h2 className="document-title">{selected}</h2><Button isIconOnly data-autofocus label="Close provider details" size="sm" variant="ghost" icon={smallIcon(X)} onClick={()=>setSelected(null)}/></HStack><HStack align="center" justify="between" gap={2}><p className="muted">{selected==='OpenAI'?'Personal account':'API key account'}</p><Status>Connected</Status></HStack><SettingsSection title="Connection" titleId="connection"><SettingsList density="compact" hasDividers><SettingsListItem label="Sign-in method" endContent={<span className="muted">{selected==='OpenAI'?'ChatGPT':'API key'}</span>}/><SettingsListItem label="Default account" endContent={<span className="muted">{selected==='OpenAI'?'Yes':'No'}</span>}/></SettingsList><SettingsSectionInset divided><B variant="secondary" icon={smallIcon(KeyRound)} onClick={()=>open('connection')}>Manage sign-in</B></SettingsSectionInset></SettingsSection><SettingsSection title="Available features" titleId="features"><SettingsList density="compact" hasDividers><SettingsListItem label="Language models" description="Use this account for chat and task models."/></SettingsList><SettingsSectionInset divided><p className="small muted">Provider: {selected.toLowerCase()} · Capability: model.generate</p></SettingsSectionInset></SettingsSection>{selected==='Anthropic'?<VStack as="section" className="compact-section" gap={2}><p className="small muted">Remove stored credentials and web tool selections for this account.</p><B variant="destructive" onClick={()=>open('remove')}>Delete account…</B></VStack>:null}</VStack>:null}/>;
}
function Notifications({notice}){
 const [enabled,setEnabled]=useState(true);
 return <VStack className="single-settings" gap={4}><p className="muted">Stay informed when Noema needs you.</p><SettingsSection title="This device" titleId="device"><SettingsList hasDividers density="balanced"><SettingsListItem label="Device notifications" description="Uses this browser’s notification permission." endContent={<Switch label="Device notifications" isLabelHidden aria-label="Device notifications" value={enabled} onChange={v=>{setEnabled(v);notice(v?'Notifications enabled in this mock.':'Notifications disabled in this mock.')}}/>}/></SettingsList><SettingsSectionInset divided><Status variant={enabled?'success':'neutral'}>{enabled?'Allowed in this browser':'Off in this browser'}</Status></SettingsSectionInset></SettingsSection><SettingsSection title="Apple device delivery" titleId="apple"><SettingsSectionInset><HStack align="start" justify="between" gap={3}><VStack gap={1}><strong>Server setup</strong><p className="small muted">Needed to send notifications to the native Apple apps.</p></VStack><Status variant="neutral">Not configured</Status></HStack></SettingsSectionInset><SettingsSectionInset divided><VStack gap={3}><p className="small muted">Provide the Apple Team ID, Key ID, and push notification key in a setup dialog.</p><B variant="secondary" onClick={()=>notice('This mock shows layout only. Apple credentials are never requested here.')}>Configure Apple push</B></VStack></SettingsSectionInset></SettingsSection></VStack>
}
function Agents({notice}){
 const [choices,setChoices]=useState({});
 const row=(name,model)=>{
  const value={model,reason:'Medium',fast:false,...choices[name]};
  const change=patch=>{setChoices(current=>({...current,[name]:{...value,...patch}}));notice(`${name} settings saved in this mock.`)};
  return <SettingsListItem key={name} label={<VStack gap={2} className="model-row">
   <strong>{name}</strong>
   <HStack gap={3} className="model-fields" align="end">
    <Selector label="Model" isLabelHidden aria-label={`${name} model`} size="sm" options={['GPT-5.6-Terra','GPT-5.6-Sol','GPT-5.6-Luna']} value={value.model} onChange={model=>change({model})}/>
    <Selector label="Reasoning" isLabelHidden aria-label={`${name} reasoning`} size="sm" options={['Low','Medium','High','XHigh']} value={value.reason} onChange={reason=>change({reason})}/>
    <Switch label="Fast" aria-label={`${name} fast mode`} value={value.fast} onChange={fast=>change({fast})}/>
   </HStack>
  </VStack>}/>;
 };
 return <VStack className="single-settings" gap={4}>
  <SettingsSection title="Agent models" titleId="agent-models"><SettingsList hasDividers density="balanced">{row('Momo','GPT-5.6-Terra')}{row('Task reviewer','GPT-5.6-Luna')}</SettingsList></SettingsSection>
  <SettingsSection title="Task models" titleId="task-models"><SettingsList hasDividers density="balanced">{row('Simple tasks','GPT-5.6-Luna')}{row('Medium tasks','GPT-5.6-Luna')}{row('High complexity tasks','GPT-5.6-Sol')}</SettingsList></SettingsSection>
  <HStack className="compact-section" gap={3} justify="between" align="center" wrap="wrap"><VStack gap={1}><h3>External task agents (ACP)</h3><p className="small muted">Connect a local agent that can carry out tasks.</p></VStack><B variant="secondary" onClick={()=>notice('Agent setup is outside this layout mock.')}>Add ACP agent</B></HStack>
 </VStack>
}
function Capture({onDone}){
 const [title,setTitle]=useState(''),[instructions,setInstructions]=useState(''),[project,setProject]=useState('none'),[folder,setFolder]=useState(''),[source,setSource]=useState(false),[pending,setPending]=useState(false);
 return <VStack className="task-document capture" gap={3}><TaskTitleField focus value={title} onChange={setTitle}/><TaskInstructionsField fill value={instructions} onChange={setInstructions} sourceMode={source} onSourceModeChange={setSource} onPendingChange={setPending}/><details className="quiet-details capture-advanced"><summary>Advanced</summary><VStack gap={3} className="capture-advanced-body"><HStack gap={3} className="schedule-pair"><Selector label="Task agent" size="sm" value="built-in" options={[{value:'built-in',label:'Noema (built-in)'}]} onChange={()=>{}}/><TextInput label="Working folder" isOptional size="sm" placeholder="Use the default folder" value={folder} onChange={setFolder}/></HStack><Switch label="Markdown source" isDisabled={pending} value={source} onChange={setSource}/></VStack></details><section className="capture-actions"><HStack gap={2} wrap="wrap" className="capture-options"><Selector label="Project" isLabelHidden size="sm" options={[{value:'none',label:'No project'},{value:'personal',label:'Personal'}]} value={project} onChange={setProject}/><B variant="ghost" icon={smallIcon(CalendarClock)} onClick={()=>location.hash='schedule'}>Schedule</B></HStack><HStack gap={2} className="capture-buttons"><B variant="secondary" isDisabled={!title.trim()} onClick={()=>onDone(title)}>Add to Inbox</B><B isDisabled={!title.trim()} onClick={()=>onDone('Queued: '+title)}>Run now</B></HStack></section></VStack>
}
function DialogTaskCard({title,recurring,active,onOpen}){
 return <ListCardButton className="history-task-card" aria-label={'Open task: '+title} onClick={onOpen}>
  <strong>{title}</strong>
  <span className="small muted">{title==='Weekly meal ideas'?'Five easy dinners and a shopping list.':title==='Compare home internet plans'?'Monthly costs, speeds, and contract terms.':title==='Find a beginner pottery class'?'Beginner classes with evening or weekend sessions.':recurring?'Three articles about design and technology.':'A relaxed weekend for two, with food and a walk.'}</span>
  <HStack gap={1} align="center">{recurring?<Status>Active</Status>:<TaskStatusBadge status={active?'executing':'queued'} stageBehavior={active?'ACTIVE':'INTAKE'}/>}<span className="small muted">{recurring?(title==='Weekly meal ideas'?'· Sundays · 10:00 AM':'· Weekdays · 8:00 AM'):title==='Compare home internet plans'?'· Home':'· Personal'}</span></HStack>
 </ListCardButton>
}
function MockDialog({type,onClose,onDone,taskTitle}){
 const [repeat,setRepeat]=useState(type==='task-schedule'?'once':'weekdays'),[zone,setZone]=useState('pacific'),[time,setTime]=useState('08:00'),[date,setDate]=useState('2026-09-07'),[message,setMessage]=useState('');
 const schedule=['schedule','task-schedule','recurrence-schedule'].includes(type);
 const config={schedule:['Edit schedule','Changes apply to future runs.','Save schedule'], 'task-schedule':['Schedule task','Plan a weekend in Portland','Schedule task'],'recurrence-schedule':['Reschedule task','Changes apply to future runs.','Save schedule'],start:['Start this task?','Plan a weekend in Portland','Start task'],cancel:['Cancel this task?','Plan a weekend in Portland','Cancel task'],end:['End this recurring task?','Morning reading','End recurring task'],'run-now':['Run this task now?','Morning reading','Run now'],connection:['Manage sign-in','Sample account controls','Done'],remove:['Delete this account?','Anthropic','Delete account'],'add-provider':['Add provider','Connect another account.','Done']}[type]||['Task details','','Done'];
 const destructive=['cancel','end','remove'].includes(type);
 const taskDialog=schedule||['start','cancel','end','run-now'].includes(type);
 const recurring=['schedule','recurrence-schedule','end','run-now'].includes(type);
 const title=taskTitle||(recurring?'Morning reading':'Plan a weekend in Portland');
 const titleSubtitle=['start','cancel','end','run-now','task-schedule'].includes(type);
 const close=()=>onClose();
 const form=<Layout height="auto" header={<DialogHeader title={config[0]} subtitle={titleSubtitle?undefined:config[1]} onOpenChange={close}/>} content={<LayoutContent><VStack gap={3}>
  {taskDialog?<DialogTaskCard title={title} recurring={recurring} active={type==='cancel'} onOpen={()=>{onClose();location.hash=recurring?'recurring':'inbox'}}/>:null}
  {schedule?<><Selector label="Repeat" size="sm" options={[...(type==='task-schedule'?[{value:'once',label:'Does not repeat'}]:[]),{value:'weekdays',label:'Every weekday'},{value:'daily',label:'Every day'},{value:'weekly',label:'Every Monday'},{value:'custom',label:'Custom schedule (cron)'}]} value={repeat} onChange={setRepeat}/><HStack gap={3} align="start" className="schedule-pair"><TextInput label="Time" type="time" size="sm" value={time} onChange={setTime}/><Selector label="Time zone" size="sm" options={[{value:'pacific',label:'Pacific time'},{value:'eastern',label:'Eastern time'}]} value={zone} onChange={setZone}/></HStack><TextInput label={repeat==='once'?'Date':'Starts on'} type="date" size="sm" value={date} onChange={setDate}/>{repeat==='custom'?<TextInput label="Cron expression" value={message} onChange={setMessage} description="Five fields: minute hour day month weekday."/>:null}<section className="schedule-preview"><strong>{repeat==='once'?'Once':repeat==='daily'?'Every day':repeat==='weekly'?'Every Monday':repeat==='custom'?'Custom schedule':'Monday to Friday'} · {time}</strong><p className="small muted">{repeat==='custom'?'Preview requires a valid cron expression.':`Preview starts ${date} · ${zone==='pacific'?'Pacific':'Eastern'} time`}</p></section>{type!=='recurrence-schedule'?<ScheduleBehavior once={repeat==='once'}/>:null}</>:<><p>{type==='start'?'Noema will plan this task and add it to the queue.':type==='cancel'?'Noema will stop this task. Saved history will remain available.':type==='end'?'No new scheduled tasks will be created. Existing tasks will remain unchanged.':type==='run-now'?'Noema will create a task now. The regular schedule will stay in place.':type==='remove'?'Stored credentials and web tool selections for this account will be removed.':'These account controls are outside this visual study.'}</p></>}
 </VStack></LayoutContent>} footer={<LayoutFooter><HStack gap={2} className="dialog-actions"><B variant="secondary" onClick={close}>{type==='cancel'?'Keep task':destructive?'Keep '+(type==='end'?'schedule':'account'):'Cancel'}</B><B variant={destructive?'destructive':'primary'} isDisabled={schedule&&repeat==='custom'&&!message.trim()} onClick={()=>onDone(config[2]+' · preview only')}>{config[2]}</B></HStack></LayoutFooter>}/>;
 return <ResponsiveDialog isOpen width={schedule?480:440} maxHeight="85dvh" purpose="form" aria-label={config[0]} onOpenChange={close}>{form}</ResponsiveDialog>
}
// The real shell reads sample projects through an in-memory link. No transport is installed.
// Unsupported mutations fail locally; this gallery cannot change app data.
let previewRouter;
if(new URLSearchParams(location.search).has('preview')){
 document.body.classList.add('shell-preview');
 const client=new ApolloClient({cache:new InMemoryCache(),link:new ApolloLink(operation=>new Observable(observer=>{
  if(operation.operationName==='TasksProjects'){
   observer.next({data:{projects:{edges:[],pageInfo:{hasNextPage:false,hasPreviousPage:false,startCursor:null,endCursor:null}}}});observer.complete();
  }else observer.error(new Error('Project changes are outside this layout study.'));
 }))});
 const rootRoute=createRootRoute({component:Preview});
 previewRouter=createRouter({routeTree:rootRoute.addChildren([createRoute({getParentRoute:()=>rootRoute,path:'$'})]),history:createMemoryHistory({initialEntries:[hrefForRoute(routeForStudy(location.hash.slice(1)))]})});
 createRoot(document.getElementById('root')).render(<ApolloProvider client={client}><MotionRoot><RouterProvider router={previewRouter}/></MotionRoot></ApolloProvider>);
}else createRoot(document.getElementById('root')).render(<App/>);
