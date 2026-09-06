from PIL import Image,ImageDraw,ImageFont
from pathlib import Path
root=Path('/var/tmp/noema-suite-run-20260905/ocr');root.mkdir(exist_ok=True)
im=Image.new('RGB',(1400,260),'white');draw=ImageDraw.Draw(im);font=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',48)
draw.text((40,40),'AUDIT INVOICE 12345',font=font,fill='black');draw.text((40,120),'TOTAL 42.00 USD',font=font,fill='black');im.save(root/'scan.png')
(root/'notes.txt').write_text('Unrelated café 日本語',encoding='utf-8')
