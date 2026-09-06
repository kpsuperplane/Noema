from pathlib import Path
from zipfile import ZipFile
import re,base64,datetime
root=Path('/var/tmp/noema-suite-run-20260905/sheets');root.mkdir(exist_ok=True)
s=Path('/root/noema/internal/documents/spreadsheet_test.go').read_text();(root/'audit.xls').write_bytes(base64.b64decode(re.search(r'const xlsFixture = "([^"]+)"',s).group(1)))
parts=dict(re.findall(r'"([^"]+)":\s*`([^`]+)`',s.split('xlsxParts := map[string]string{',1)[1].split('\n\t}',1)[0]))
parts['xl/styles.xml']='<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="2"><numFmt numFmtId="164" formatCode="yyyy-mm-dd"/><numFmt numFmtId="165" formatCode="$#,##0.00"/></numFmts><fonts count="1"><font/></fonts><fills count="1"><fill/></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0"/><xf numFmtId="164" applyNumberFormat="1"/><xf numFmtId="165" applyNumberFormat="1"/></cellXfs></styleSheet>'
serial=(datetime.date(2026,9,6)-datetime.date(1899,12,30)).days
parts['xl/worksheets/sheet1.xml']=f'<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row><c r="A1" t="inlineStr"><is><t>Date</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c><c r="C1" t="inlineStr"><is><t>Formula</t></is></c></row><row><c r="A2" s="1"><v>{serial}</v></c><c r="B2" s="2"><v>1234.5</v></c><c r="C2"><f>6*7</f><v>42</v></c></row></sheetData></worksheet>'
with ZipFile(root/'audit.xlsx','w') as z:
    for name,value in parts.items():z.writestr(name,value)
with ZipFile(root/'audit.ods','w') as z:
    z.writestr('content.xml','<office:document-content xmlns:office="office" xmlns:table="table" xmlns:text="text"><office:body><office:spreadsheet><table:table table:name="Audit"><table:table-row><table:table-cell office:value-type="string"><text:p>Label café 日本語</text:p></table:table-cell><table:table-cell office:value-type="date" office:date-value="2026-09-06"><text:p>2026-09-06</text:p></table:table-cell><table:table-cell office:value-type="currency" office:currency="USD" office:value="1234.5"><text:p>$1,234.50</text:p></table:table-cell><table:table-cell office:value-type="float" office:value="42" table:formula="of:=6*7"><text:p>42</text:p></table:table-cell></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>')
