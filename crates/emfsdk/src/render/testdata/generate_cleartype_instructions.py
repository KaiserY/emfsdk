"""Generate project-owned TrueType instruction controls (requires fontTools)."""
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables.ttProgram import Program
from fontTools.ttLib import newTable
from pathlib import Path
import json
root=Path(__file__).resolve().parent
order=['.notdef']+[f'g{i}' for i in range(64)];glyphs={};metrics={};cases=[]
def push(*a):return ['PUSHW[ ]',*map(str,a)]
for j,name in enumerate(order):
 i=j-1;kind=i//4;phase=(i%4)*8;pen=TTGlyphPen(None);prog=[]
 if j:
  x=98+phase;width=16 if kind==5 else 128;y=18+phase if kind==10 else 0
  pen.moveTo((x,320+y));pen.lineTo((x+width,320+y));pen.lineTo((x+width,y));pen.lineTo((x,y));pen.closePath()
  prog=['SVTCA[1]']
  if kind==0:prog+=push(0)+['MDAP[1]']
  elif kind==1:prog+=push(0)+['MDAP[0]']+push(1)+['MDRP[00100]']
  elif kind==2:prog+=push(0,0)+['MIAP[1]']
  elif kind in (3,4):prog+=push(0)+['MDAP[0]']+push(1,0)+['MIRP[00100]' if kind==3 else 'MIRP[00000]']
  elif kind==5:prog+=push(0)+['MDAP[0]']+push(1)+['MDRP[01100]']
  elif kind in (6,7):prog+=['SVTCA[0]']+push(0)+['MDAP[0]','SVTCA[1]']+push(0)+['MDAP[0]']
  if kind==6:prog+=push(0,32)+['SHPIX[ ]']
  elif kind==7:prog+=push(247,0,1)+['DELTAP3[ ]'] # 17 + 32 + 15 = 64 ppem; prep sets delta_base to 17
  elif kind==8:prog+=push(0,98+phase)+['ROUND[00]','SCFS[ ]']
  elif kind==9:prog+=push(0,1)+['MIAP[0]'] # prep-rounded CVT1
  elif kind==10:prog=['SVTCA[0]']+push(2)+['MDAP[1]','IUP[0]']
  elif kind==11:prog+=push(0)+['MDAP[0]']+push(1,192)+['MSIRP[0]']
  elif kind==12:prog+=push(0)+['MDAP[1]']+push(1)+['SHP[1]']
  elif kind==13:prog+=['RDTG[ ]']+push(0)+['MDAP[1]']
  elif kind==14:prog+=['RUTG[ ]']+push(0)+['MDAP[1]']
  elif kind==15:prog+=['RTHG[ ]']+push(0)+['MDAP[1]']
  prog+=['IUP[1]']
  cases.append({'glyph':name,'kind':kind,'phase':phase,'program':prog})
 g=pen.glyph();g.program=Program();g.program.fromAssembly(prog);glyphs[name]=g;metrics[name]=(512,0 if not j else x)
fb=FontBuilder(4096,isTTF=True);fb.setupGlyphOrder(order);fb.setupCharacterMap({33+i:f'g{i}' for i in range(64)});fb.setupGlyf(glyphs);fb.setupHorizontalMetrics(metrics);fb.setupHorizontalHeader(ascent=4096,descent=0)
f='OOXML GDI vmops';fb.setupNameTable({'familyName':f,'styleName':'Regular','uniqueFontIdentifier':f+' 1.0','fullName':f,'psName':'OOXMLGDIvmops','version':'Version 1.000'});fb.setupOS2(sTypoAscender=4096,sTypoDescender=0,usWinAscent=4096,usWinDescent=0);fb.setupPost();fb.setupMaxp();font=fb.font;font['head'].flags=3;font['maxp'].maxStackElements=32
import array
cvt=newTable('cvt ');cvt.values=array.array('h',[192,98]);font['cvt ']=cvt
prep=newTable('prep');prep.program=Program();prep.program.fromAssembly(push(1,1)+['RCVT[ ]','ROUND[00]','WCVTP[ ]']+push(17)+['SDB[ ]']);font['prep']=prep
# The checked-in PNG is an independent Windows capture; do not regenerate it here.
font.save(root/'cleartype-instructions.ttf')
