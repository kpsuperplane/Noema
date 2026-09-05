import React, {useState, useEffect} from 'react';
import {createRoot} from 'react-dom/client';
import {Button} from '@astryxdesign/core/Button';
import {IconButton} from '@astryxdesign/core/IconButton';
import {HStack} from '@astryxdesign/core/HStack';
import {VStack} from '@astryxdesign/core/VStack';
import {Selector} from '@astryxdesign/core/Selector';
import {TextInput} from '@astryxdesign/core/TextInput';
import {TextArea} from '@astryxdesign/core/TextArea';
import {DropdownMenu} from '@astryxdesign/core/DropdownMenu';
import {Switch} from '@astryxdesign/core/Switch';
import {Dialog as ResponsiveDialog} from '@/components/ResponsiveDialog';
import {Dialog, DialogHeader} from '@astryxdesign/core/Dialog';
import {Layout, LayoutContent, LayoutFooter} from '@astryxdesign/core/Layout';
import {StatusDot} from '@astryxdesign/core/StatusDot';
import {Inbox, Repeat2, CalendarClock, Play, Pause, ArrowLeft, Plus, Check, Pencil, Ellipsis, KeyRound} from 'lucide-react';
import {TaskStatusBadge} from '@/components/chatDetail/task/TaskStatusBadge';
import {ListCardButton} from '@/components/ListCardLink';
import {SettingsSection, SettingsList, SettingsListItem, SettingsSectionInset} from '@/components/settings/SettingsPrimitives';
import '@astryxdesign/core/reset.css';
import '@astryxdesign/core/astryx.css';
import '@astryxdesign/theme-neutral/theme.css';
import '@/theme/noema-neutral.css';
import './gallery.css';

const studies = [
 ['inbox','Inbox task','01','Lead with the task, its source, and what happens next.','The current detail uses “No agent run yet” and “No output yet.” The mock gives an unstarted task a useful summary.'],
 ['recurring','Recurring task','02','Make the next run clear. Make past runs useful.','Current rows repeat “Open task.” Outcome labels below need linked task status; they must not be inferred from occurrence creation.'],
 ['schedule','Schedule dialog','03','Edit a routine without learning cron.','The current editor initializes every recurrence as Custom cron. Preserve matching presets and show one compact schedule preview.'],
 ['providers','Provider settings','04','Put connection health and useful facts first.','Current details separate Authentication, Status, and Default account into tall rows. Capability IDs and contract fields remain prominent.'],
 ['notifications','Notification settings','05','Start with this device. Keep server setup available.','Apple push configuration currently appears before device notifications. The mock keeps both functions on the same page.'],
 ['task-settings','Advanced task settings','06','Keep task controls where you use them.','Advanced settings stay visible below instructions. Inbox and recurring tasks share the layout, with fields that match each task type.'],
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
function App(){
 const [id,setId]=useState(location.hash.slice(1)||'inbox');
 const [phone,setPhone]=useState(false);
 useEffect(()=>{const change=()=>setId(studies.some(s=>s[0]===location.hash.slice(1))?location.hash.slice(1):'inbox');window.addEventListener('hashchange',change);return()=>window.removeEventListener('hashchange',change)},[]);
 const study=studies.find(s=>s[0]===id)||studies[0];
 return <VStack className="gallery" gap={4}>
  <HStack as="header" justify="between" align="center" wrap="wrap" gap={3} className="gallery-header">
   <VStack gap={0.5}><strong className="gallery-brand">Noema <span> / Core UI study</span></strong><p className="small muted">Proposed refinements · Sample data · UI unchanged</p></VStack>
   <B variant="secondary" aria-pressed={phone} onClick={()=>setPhone(!phone)}>{phone?'Desktop width':'Phone width'}</B>
  </HStack>
  <nav className="study-nav" aria-label="Mock gallery">{studies.map(s=><a key={s[0]} href={'#'+s[0]} aria-current={s[0]===id?'page':undefined}><span>{s[2]}</span>{s[1]}</a>)}</nav>
  <HStack justify="between" align="center" gap={3} className="study-heading"><h1>{study[3]}</h1><span className="small muted">{study[2]} / 10</span></HStack>
  <section className={'preview '+(phone?'phone':'')} aria-label={study[1]+' mock'} key={id+phone}>
   <Scene id={id}/>
  </section>
  <aside className="annotation"><strong>Why this change</strong><p>{study[4]}</p><p className="small muted">Based on source and read-only app inspection. Controls change only this mock. The app’s navigation and route structure stay as they are.</p></aside>
 </VStack>
}
function Scene({id}){
 const [modal,setModal]=useState(null),[notice,setNotice]=useState(''),[paused,setPaused]=useState(false),[selected,setSelected]=useState(0),[started,setStarted]=useState(false),[cancelled,setCancelled]=useState(false),[detail,setDetail]=useState(false);
 const [savedTitle,setSavedTitle]=useState('');
 const done=(message)=>{setNotice(message);setModal(null)};
 const dialogs=['schedule','start','cancel'];
 const open=type=>setModal(type);
 const isDialog=dialogs.includes(id);
 const isSettings=['providers','notifications','agents'].includes(id);
 return <>
  <header className="surface-header"><HStack align="center" gap={2}>{detail?<B variant="ghost" icon={smallIcon(ArrowLeft)} onClick={()=>setDetail(false)}>Back</B>:null}<h2>{isSettings?'Settings':id==='recurring'||id==='schedule'?'Recurring tasks':'Tasks'}</h2></HStack><span className="small muted">{isSettings?(id==='providers'?'Providers':id==='agents'?'Agents':'Notifications'):'Personal workspace'}</span></header>
  {isDialog?<VStack className="dialog-stage" align="center" justify="center" gap={3}><MockDialog type={id} inline onClose={()=>setNotice('Use “Open as modal” to preview dismissal.')} onDone={message=>setNotice(message)}/><p role="status" className="small muted">{notice||'Dialog preview · Changes stay in this gallery'}</p><B variant="ghost" onClick={()=>open(id)}>Open as modal</B></VStack>:id==='agents'?<Agents notice={setNotice}/>:id==='providers'?<Provider open={open}/>:id==='notifications'?<Notifications notice={setNotice}/>:<section className={'task-split '+(detail?'show-detail':'')}>
   <aside className="task-list"><HStack justify="between" align="center"><h3>{id==='recurring'?'Recurring': 'Inbox'}</h3><B variant="ghost" icon={smallIcon(Plus)} onClick={()=>location.hash='capture'}>New task</B></HStack><p className="small muted list-intro">{id==='recurring'?'Tasks that run on a schedule.':'Saved here until you start or schedule them.'}</p><VStack gap={2}>{(id==='recurring'?[['Morning reading','Weekdays · 8:00 AM'],['Weekly meal ideas','Sundays · 10:00 AM']]:samples).map((t,i)=><ListCardButton key={t[0]} selected={selected===i} onClick={()=>{setSelected(i);setDetail(true)}}><HStack gap={2} align="start">{smallIcon(id==='recurring'?Repeat2:Inbox)}<VStack as="span" gap={1}><strong>{t[0]}</strong><span className="small muted">{t[1]}</span></VStack></HStack></ListCardButton>)}</VStack></aside>
   <section className="task-detail">
    {id==='recurring'?<Recurrence notice={setNotice} title={selected?'Weekly meal ideas':'Morning reading'} paused={paused} toggle={()=>{setPaused(!paused);setNotice(paused?'Schedule resumed in this mock.':'Schedule paused in this mock.')}} open={open}/>:id==='capture'?<Capture onDone={m=>{setSavedTitle(m);done('Task saved in this mock.')}}/>:<InboxDetail key={selected} selected={selected} started={started} cancelled={cancelled} open={open} onStart={()=>{setStarted(true);setNotice('Task queued in this mock.')}}/>}
   </section>
  </section>}
  {notice&&!isDialog?<p className="mock-notice" role="status"><Check size={14}/>{notice}{savedTitle?' '+savedTitle:''}</p>:null}
  {modal?<MockDialog type={modal} onClose={()=>setModal(null)} onDone={message=>{if(modal==='start')setStarted(true);if(modal==='cancel')setCancelled(true);done(message)}}/>:null}
 </>
}
// Shared mock sections follow the existing TaskBody and TaskContextCard composition.
function TaskDocument({title,metadata,timing,instructions,note,openInstructions,advanced,children}){
 return <VStack className="task-document" gap={4}>
  <VStack gap={2}><h2 className="document-title">{title}</h2><p className="small muted">{metadata}</p></VStack>
  {timing}
  <VStack as="section" gap={2}><HStack justify="between" align="center" gap={2}><h3>Instructions</h3><B variant="ghost" icon={smallIcon(Pencil)} onClick={openInstructions}>Edit</B></HStack>{instructions}<p className="small muted">{note}</p></VStack>
  {advanced}
  {children}
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
function AdvancedTaskSettings({recurring=false}){
 const [project,setProject]=useState('personal'),[agent,setAgent]=useState('built-in'),[folder,setFolder]=useState(''),[savedFolder,setSavedFolder]=useState(''),[saved,setSaved]=useState('');
 return <VStack as="section" className="compact-section task-advanced" gap={2} aria-label="Advanced settings">
  <HStack justify="between" align="center" gap={2}><h3>Advanced settings</h3><span role="status" className="small muted">{saved}</span></HStack>
  {recurring?<ScheduleBehavior onSaved={()=>setSaved('Saved in this mock')}/>:<>
   <HStack gap={3} className="schedule-pair"><Selector label="Project" size="sm" value={project} options={[{value:'personal',label:'Personal'},{value:'none',label:'No project'}]} onChange={v=>{setProject(v);setSaved('Saved in this mock')}}/><Selector label="Task agent" size="sm" value={agent} options={[{value:'built-in',label:'Noema (built-in)'},{value:'local',label:'Local agent (ACP)'}]} onChange={v=>{setAgent(v);setSaved('Saved in this mock')}}/></HStack>
   <TextInput label="Working folder" isOptional size="sm" placeholder="Use the project folder or Noema’s Tasks folder" value={folder} onChange={v=>{setFolder(v);setSaved('')}} onBlur={()=>{if(folder!==savedFolder){setSavedFolder(folder);setSaved('Saved in this mock')}}}/>
  </>}
 </VStack>
}
function ScheduleBehavior({onSaved=()=>{},once=false}){
 const [missed,setMissed]=useState('run_once'),[overlap,setOverlap]=useState('skip');
 return <VStack gap={3}>
  <Selector size="sm" label="If Noema misses a run" value={missed} options={[{value:'run_once',label:'Run once when Noema is available'},{value:'skip',label:'Skip the missed run'}]} onChange={v=>{setMissed(v);onSaved()}}/>
  {!once?<Selector size="sm" label="If the previous run is still active" value={overlap} options={[{value:'skip',label:'Skip the next run'},{value:'queue',label:'Queue one run'},{value:'allow',label:'Allow both runs'}]} onChange={v=>{setOverlap(v);onSaved()}}/>:null}
 </VStack>
}
function InboxDetail({selected,started,cancelled,open,onStart}){
 return <>
  <TaskDocument title={samples[selected][0]} metadata={'Inbox · '+samples[selected][1]+' · From you'}
   timing={<TaskTiming label="Timing" value="No scheduled start" onSchedule={()=>open('task-schedule')}/>}
   instructions={<><p>{selected===0?'Find a relaxed weekend plan for two people. Include places to eat, a walk, and one indoor option.':selected===1?'Compare monthly costs, speeds, and contract terms. Keep the recommendations short.':'Find beginner classes with evening or weekend sessions. Include prices and what materials are provided.'}</p>{selected===0?<p>Keep the total budget under $400. Include links so I can review the options.</p>:null}</>}
   note="Created Sep 5, 2026 · Revision 1" openInstructions={()=>open('instructions')} advanced={<AdvancedTaskSettings/>}/>
  <TaskDock status={cancelled?'Task cancelled':started?'Queued':'Ready when you are'}><IconButton className="task-dock-control" size="sm" variant="ghost" icon={<Play size={15} aria-hidden="true" color="var(--noema-pine-700)" fill="var(--noema-pine-700)"/>} label={started?'Queued':'Start task'} tooltip={started?'Queued':'Start task'} isDisabled={started||cancelled} onClick={onStart}/></TaskDock>
 </>
}
function Recurrence({title,paused,toggle,open,notice}){
 const [run,setRun]=useState(null);
 return <>
 <TaskDocument title={title} metadata={(title==='Morning reading'?'Weekdays at 8:00 AM':'Sundays at 10:00 AM')+' · Pacific time'}
  timing={<TaskTiming label={paused?'Schedule paused':'Next run'} value={paused?'No new scheduled runs':title==='Morning reading'?'Monday, Sep 7 · 8:00 AM':'Sunday, Sep 6 · 10:00 AM'} paused={paused} scheduled onSchedule={()=>open('recurrence-schedule')}/>}
  instructions={<p>{title==='Morning reading'?'Find three thoughtful articles about design and technology. Give me a short summary and a link for each.':'Suggest five easy dinners for next week. Include a shopping list and keep preparation under 30 minutes.'}</p>}
  note="Edits apply to future runs." openInstructions={()=>open('recurrence-instructions')} advanced={<AdvancedTaskSettings recurring/>}>
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
 const [selected,setSelected]=useState('OpenAI');
 return <section className="settings-split"><aside className="provider-list"><HStack justify="between" align="center"><h3>Provider accounts</h3><B variant="ghost" icon={smallIcon(Plus)} onClick={()=>open('add-provider')}>Add</B></HStack><VStack gap={2}>{['OpenAI','Anthropic'].map(n=><ListCardButton key={n} selected={selected===n} onClick={()=>setSelected(n)}><HStack align="center" justify="between"><VStack as="span" gap={1}><strong>{n}</strong><span className="small muted">{n==='OpenAI'?'Personal account':'API key'}</span></VStack><Status>Connected</Status></HStack></ListCardButton>)}</VStack></aside><VStack className="settings-content" gap={4}><HStack align="center" justify="between"><VStack gap={1}><h2 className="document-title">{selected}</h2><p className="muted">{selected==='OpenAI'?'Personal account':'API key account'}</p></VStack><Status>Connected</Status></HStack><SettingsSection title="Connection" titleId="connection"><SettingsList density="compact" hasDividers><SettingsListItem label="Sign-in method" endContent={<span className="muted">{selected==='OpenAI'?'ChatGPT':'API key'}</span>}/><SettingsListItem label="Default account" endContent={<span className="muted">{selected==='OpenAI'?'Yes':'No'}</span>}/></SettingsList><SettingsSectionInset divided><B variant="secondary" icon={smallIcon(KeyRound)} onClick={()=>open('connection')}>Manage sign-in</B></SettingsSectionInset></SettingsSection><SettingsSection title="Available features" titleId="features"><SettingsList density="compact" hasDividers><SettingsListItem label="Language models" description="Use this account for chat and task models."/></SettingsList><SettingsSectionInset divided><p className="small muted">Provider: {selected.toLowerCase()} · Capability: model.generate</p></SettingsSectionInset></SettingsSection>{selected==='Anthropic'?<VStack as="section" className="compact-section" gap={2}><p className="small muted">Remove stored credentials and web tool selections for this account.</p><B variant="destructive" onClick={()=>open('remove')}>Delete account…</B></VStack>:null}</VStack></section>
}
function Notifications({notice}){
 const [enabled,setEnabled]=useState(true);
 return <VStack className="single-settings" gap={4}><VStack gap={1}><h2 className="document-title">Notifications</h2><p className="muted">Stay informed when Noema needs you.</p></VStack><SettingsSection title="This device" titleId="device"><SettingsList hasDividers density="balanced"><SettingsListItem label="Device notifications" description="Uses this browser’s notification permission." endContent={<Switch label="Device notifications" aria-label="Device notifications" value={enabled} onChange={v=>{setEnabled(v);notice(v?'Notifications enabled in this mock.':'Notifications disabled in this mock.')}}/>}/></SettingsList><SettingsSectionInset divided><Status variant={enabled?'success':'neutral'}>{enabled?'Allowed in this browser':'Off in this browser'}</Status></SettingsSectionInset></SettingsSection><SettingsSection title="Apple device delivery" titleId="apple"><SettingsSectionInset><HStack align="start" justify="between" gap={3}><VStack gap={1}><strong>Server setup</strong><p className="small muted">Needed to send notifications to the native Apple apps.</p></VStack><Status variant="neutral">Not configured</Status></HStack></SettingsSectionInset><SettingsSectionInset divided><VStack gap={3}><p className="small muted">Provide the Apple Team ID, Key ID, and push notification key in a setup dialog.</p><B variant="secondary" onClick={()=>notice('This mock shows layout only. Apple credentials are never requested here.')}>Configure Apple push</B></VStack></SettingsSectionInset></SettingsSection></VStack>
}
function Agents({notice}){
 const [choices,setChoices]=useState({});
 const row=(name,model,task=false)=>{
  const value={model,reason:'Medium',fast:false,enabled:true,...choices[name]};
  const change=patch=>{setChoices(current=>({...current,[name]:{...value,...patch}}));notice(`${name} settings saved in this mock.`)};
  return <SettingsListItem key={name} label={<VStack gap={2} className="model-row">
   <HStack align="center" justify="between" gap={2}><strong>{name}</strong>{task?<Switch label={value.enabled?'Enabled':'Off'} aria-label={`${name} enabled`} value={value.enabled} onChange={enabled=>change({enabled})}/>:null}</HStack>
   <HStack gap={3} className="model-fields" align="end">
    <Selector label="Model" isLabelHidden aria-label={`${name} model`} size="sm" options={['GPT-5.6-Terra','GPT-5.6-Sol','GPT-5.6-Luna']} value={value.model} onChange={model=>change({model})}/>
    <Selector label="Reasoning" isLabelHidden aria-label={`${name} reasoning`} size="sm" options={['Low','Medium','High','XHigh']} value={value.reason} onChange={reason=>change({reason})}/>
    <Switch label="Fast" aria-label={`${name} fast mode`} value={value.fast} onChange={fast=>change({fast})}/>
   </HStack>
  </VStack>}/>;
 };
 return <VStack className="single-settings" gap={4}><h2 className="document-title">Agents</h2>
  <SettingsSection title="Agent models" titleId="agent-models"><SettingsList hasDividers density="balanced">{row('Momo','GPT-5.6-Terra')}{row('Task reviewer','GPT-5.6-Luna')}</SettingsList></SettingsSection>
  <SettingsSection title="Task models" titleId="task-models"><SettingsList hasDividers density="balanced">{row('Simple tasks','GPT-5.6-Luna',true)}{row('Medium tasks','GPT-5.6-Luna',true)}{row('High complexity tasks','GPT-5.6-Sol',true)}</SettingsList></SettingsSection>
  <HStack className="compact-section" gap={3} justify="between" align="center" wrap="wrap"><VStack gap={1}><h3>External task agents (ACP)</h3><p className="small muted">Connect a local agent that can carry out tasks.</p></VStack><B variant="secondary" onClick={()=>notice('Agent setup is outside this layout mock.')}>Add ACP agent</B></HStack>
 </VStack>
}
function Capture({onDone}){
 const [title,setTitle]=useState(''),[instructions,setInstructions]=useState(''),[project,setProject]=useState('none'),[folder,setFolder]=useState(''),[source,setSource]=useState(false);
 return <VStack className="task-document capture" gap={3}><input aria-label="Task title" id="capture-title" className="capture-title" placeholder="What needs to be done?" value={title} onChange={e=>setTitle(e.target.value)}/><textarea aria-label="Instructions" id="capture-instructions" className="capture-instructions" placeholder="Add details or instructions…" value={instructions} onChange={e=>setInstructions(e.target.value)}/><details className="quiet-details capture-advanced"><summary>Advanced</summary><VStack gap={3} className="capture-advanced-body"><HStack gap={3} className="schedule-pair"><Selector label="Task agent" size="sm" value="built-in" options={[{value:'built-in',label:'Noema (built-in)'}]} onChange={()=>{}}/><TextInput label="Working folder" isOptional size="sm" placeholder="Use the default folder" value={folder} onChange={setFolder}/></HStack><Switch label="Markdown source" value={source} onChange={setSource}/></VStack></details><section className="capture-actions"><HStack gap={2} wrap="wrap" className="capture-options"><Selector label="Project" isLabelHidden size="sm" options={[{value:'none',label:'No project'},{value:'personal',label:'Personal'}]} value={project} onChange={setProject}/><B variant="ghost" icon={smallIcon(CalendarClock)} onClick={()=>location.hash='schedule'}>Schedule</B></HStack><HStack gap={2} className="capture-buttons"><B variant="secondary" isDisabled={!title.trim()} onClick={()=>onDone(title)}>Add to Inbox</B><B isDisabled={!title.trim()} onClick={()=>onDone('Queued: '+title)}>Run now</B></HStack></section></VStack>
}
function MockDialog({type,inline=false,onClose,onDone}){
 const [repeat,setRepeat]=useState(type==='task-schedule'?'once':'weekdays'),[zone,setZone]=useState('pacific'),[time,setTime]=useState('08:00'),[date,setDate]=useState('2026-09-07'),[message,setMessage]=useState('');
 const schedule=['schedule','task-schedule','recurrence-schedule'].includes(type),instructions=['instructions','recurrence-instructions'].includes(type);
 const config={schedule:['Edit schedule','Changes apply to future runs.','Save schedule'], 'task-schedule':['Schedule task','Plan a weekend in Portland','Schedule task'],'recurrence-schedule':['Reschedule task','Changes apply to future runs.','Save schedule'],start:['Start this task?','Plan a weekend in Portland','Start task'],cancel:['Cancel this task?','Plan a weekend in Portland','Cancel task'],end:['End this recurring task?','Morning reading','End recurring task'],'run-now':['Run this task now?','Morning reading','Run now'],'recurrence-instructions':['Edit instructions','Changes apply to future runs.','Save instructions'],instructions:['Edit instructions','Changes apply to this task.','Save instructions'],connection:['Manage sign-in','Sample account controls','Done'],remove:['Delete this account?','Anthropic','Delete account'],'add-provider':['Add provider','Connect another account.','Done']}[type]||['Task details','','Done'];
 const destructive=['cancel','end','remove'].includes(type);
 const close=()=>onClose();
 const form=<Layout height="auto" header={<DialogHeader title={config[0]} subtitle={config[1]} onOpenChange={close}/>} content={<LayoutContent><VStack gap={3}>
  {schedule?<><Selector label="Repeat" size="sm" options={[...(type==='task-schedule'?[{value:'once',label:'Does not repeat'}]:[]),{value:'weekdays',label:'Every weekday'},{value:'daily',label:'Every day'},{value:'weekly',label:'Every Monday'},{value:'custom',label:'Custom schedule (cron)'}]} value={repeat} onChange={setRepeat}/><HStack gap={3} align="start" className="schedule-pair"><TextInput label="Time" type="time" size="sm" value={time} onChange={setTime}/><Selector label="Time zone" size="sm" options={[{value:'pacific',label:'Pacific time'},{value:'eastern',label:'Eastern time'}]} value={zone} onChange={setZone}/></HStack><TextInput label={repeat==='once'?'Date':'Starts on'} type="date" size="sm" value={date} onChange={setDate}/>{repeat==='custom'?<TextInput label="Cron expression" value={message} onChange={setMessage} description="Five fields: minute hour day month weekday."/>:null}<section className="schedule-preview"><strong>{repeat==='once'?'Once':repeat==='daily'?'Every day':repeat==='weekly'?'Every Monday':repeat==='custom'?'Custom schedule':'Monday to Friday'} · {time}</strong><p className="small muted">{repeat==='custom'?'Preview requires a valid cron expression.':`Preview starts ${date} · ${zone==='pacific'?'Pacific':'Eastern'} time`}</p></section>{type!=='recurrence-schedule'?<ScheduleBehavior once={repeat==='once'}/>:null}</>:instructions?<TextArea label="Instructions" hasAutoFocus value={message} onChange={setMessage} placeholder="Add details or instructions…"/>:<><p>{type==='start'?'Noema will plan this task and add it to the queue.':type==='cancel'?'Noema will stop this task. Saved history will remain available.':type==='end'?'No new scheduled tasks will be created. Existing tasks will remain unchanged.':type==='run-now'?'Noema will create a task now. The regular schedule will stay in place.':type==='remove'?'Stored credentials and web tool selections for this account will be removed.':'These account controls are outside this visual study.'}</p>{type==='cancel'?<TextArea label="Reason" isOptional value={message} onChange={setMessage}/>:null}</>}
 </VStack></LayoutContent>} footer={<LayoutFooter><HStack gap={2} className="dialog-actions"><B variant="secondary" onClick={close}>{type==='cancel'?'Keep task':destructive?'Keep '+(type==='end'?'schedule':'account'):'Cancel'}</B><B variant={destructive?'destructive':'primary'} isDisabled={schedule&&repeat==='custom'&&!message.trim()} onClick={()=>onDone(config[2]+' · preview only')}>{config[2]}</B></HStack></LayoutFooter>}/>;
 const Frame=inline?Dialog:ResponsiveDialog;
 return <Frame isOpen isInline={inline} width={schedule?480:440} maxHeight="85dvh" purpose="form" aria-label={config[0]} onOpenChange={close}>{form}</Frame>
}
createRoot(document.getElementById('root')).render(<App/>);
