from pathlib import Path
from zipfile import ZipFile
root=Path('/var/tmp/noema-suite-run-20260905/pdf-epub');root.mkdir(exist_ok=True)
objects=['<< /Type /Catalog /Pages 2 0 R >>','','<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'];kids=[]
for text in ['First audit page','Second audit page']:
    page=len(objects)+1;kids.append(f'{page} 0 R');stream=f'BT /F1 12 Tf 72 720 Td ({text}) Tj ET'
    objects.extend([f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {page+1} 0 R >>',f'<< /Length {len(stream)} >>\nstream\n{stream}\nendstream'])
objects[1]=f'<< /Type /Pages /Kids [{" ".join(kids)}] /Count 2 >>'
data=b'%PDF-1.4\n';offsets=[]
for i,obj in enumerate(objects,1):
    offsets.append(len(data));data+=f'{i} 0 obj\n{obj}\nendobj\n'.encode()
xref=len(data);data+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'.encode()
for offset in offsets:data+=f'{offset:010d} 00000 n \n'.encode()
data+=f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
(root/'資料.pdf').write_bytes(data)
(root/'broken.pdf').write_bytes(b'%PDF-1.7\nnot a PDF')
with ZipFile(root/'資料.epub','w') as z:
    z.writestr('mimetype','application/epub+zip')
    z.writestr('META-INF/container.xml','<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>')
    z.writestr('OPS/book.opf','<package><metadata><title>Audit book</title></metadata><manifest><item id="one" href="one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="two.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="two"/><itemref idref="one"/></spine></package>')
    z.writestr('OPS/one.xhtml','<html><body><p>First café 日本語</p></body></html>')
    z.writestr('OPS/two.xhtml','<html><body><p>Second café 日本語</p></body></html>')
