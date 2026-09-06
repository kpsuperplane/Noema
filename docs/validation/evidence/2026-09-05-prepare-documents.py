from pathlib import Path
import re, zipfile, json, os
root=Path('/var/tmp/noema-suite-run-20260905')
fixtures=root/'Document fixtures 日本語'
fixtures.mkdir(exist_ok=True)
source=(root/'source/internal/documents/document_test.go').read_text()
cases=[]
for name,ext,want in [('DOCXKeepsStructure','docx','# Intro\n\n- \\*first\\*\n\n| Name | Count |\n| --- | --- |\n| A\\|B | 2 |'),('ODTKeepsListsAndTables','odt','## Plan\n\n- One  item\n\n| A | B |\n| --- | --- |'),('ODPKeepsTitlesAndNotes','odp','## Deck title\n\nBody\n\n> Remember this')]:
    part=source.split('func TestDocumentMarkdown'+name+'(')[1].split('\nfunc ')[0]
    key,xml=re.search(r'"([^"]+)":\s*`([^`]+)`',part).groups()
    filename='資料.'+ext
    with zipfile.ZipFile(fixtures/filename,'w',zipfile.ZIP_DEFLATED) as z: z.writestr(key,xml)
    cases.append(dict(path=filename,want=want))
for filename,content,want in [('資料.rtf',r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Arial;}}Hello \'80 \u8212? {\*\comment hidden}\par Next}",'Hello € —\n\nNext'),('資料.txt','café 日本語 🧭\n\nhttps://example.com/a?b=c','café 日本語 🧭\n\nhttps://example.com/a?b=c')]:
    (fixtures/filename).write_text(content)
    cases.append(dict(path=filename,want=want))
(fixtures/'broken.docx').write_bytes(b'not a document')
(fixtures/'bad.txt').write_bytes(b'hello\xff')
(fixtures/'outside.txt').symlink_to('/etc/hostname') if not (fixtures/'outside.txt').is_symlink() else None
(root/'document-cases.json').write_text(json.dumps(cases,ensure_ascii=False,indent=2)+'\n')
os.chmod(fixtures,0o755)
