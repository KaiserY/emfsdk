"""Project-owned compatible-width controls; native expectations are captured separately."""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import newTable
from fontTools.ttLib.tables.ttProgram import Program
import array

ROOT = Path(__file__).resolve().parent
POINTS = [(301,384),(133,0),(59,0),(129,64),(8,384),(67,384),(161,128),(259,384)]
def push(*args):return ['PUSHW[ ]',*map(str,args)]
def shp(parent=9,child=0):return push(parent)+['SRP1[ ]']+push(child)+['SHP[1]']
def scfs(point,value):return push(point,value)+['SCFS[ ]']
def ip(a=4,b=0,p=1):return push(a)+['SRP1[ ]']+push(b)+['SRP2[ ]']+push(p)+['IP[ ]']
IUP=['IUP[1]']
CASES = [
 ('no-relative-link',push(0)+['MDAP[1]']+IUP),
 ('shift-before-iup',shp()),
 ('shift',shp()+IUP),
 ('repeated-iup',shp()+IUP+IUP),
 ('left-root',shp(8)+IUP),
 ('outline-root',shp(4)+IUP),
 ('absolute-before-link',scfs(0,192)+shp()+IUP),
 ('absolute-after-link',shp()+scfs(0,192)+IUP),
 ('untouched-child',shp()+push(0)+['UTP[ ]']+IUP),
 ('repeated-link',shp()+shp()+IUP),
 ('forward-shift',shp(0,1)+shp()+IUP),
 ('reversed-chain',shp(1,6)+shp(0,1)+shp()+IUP),
 ('first-zero-parent',shp(4)+shp()+IUP),
 ('first-width-parent',shp()+shp(4)+IUP),
 ('interpolation',shp()+ip()+IUP),
 ('reversed-interpolation',shp()+ip(0,4)+IUP),
 ('forward-interpolation',ip()+shp()+IUP),
 ('first-interpolation',shp()+ip()+shp(9,1)+IUP),
 ('first-shift',shp()+shp(9,1)+ip()+IUP),
 ('phantom-interpolation',ip(8,9,0)+IUP),
 ('relative-grey',push(9)+['SRP0[ ]']+push(0)+['MDRP[00000]']+IUP),
 ('relative-black',push(9)+['SRP0[ ]']+push(0)+['MDRP[00001]']+IUP),
 ('relative-white',push(9)+['SRP0[ ]']+push(0)+['MDRP[00010]']+IUP),
 ('align-relative',push(9)+['SRP0[ ]']+push(0)+['ALIGNRP[ ]']+IUP),
 ('shift-contour',push(9)+['SRP1[ ]']+push(0)+['SHC[1]']+IUP),
]
for width in (320,336,352,384,400):
 CASES.append((f'width-before-{width}',scfs(9,width)+shp()+IUP))
 CASES.append((f'width-after-{width}',shp()+IUP+scfs(9,width)))

cases=[]
for v in (192,284,298,300,302,316,332,364):
 for mdap in (False,True):cases.append((f'root-mdap{int(mdap)}-{v}',(push(0)+['MDAP[1]'] if mdap else [])+scfs(0,v)+IUP))
for p in (0,1,4):
 for opcode in ('MIRP[00000]','MIRP[00100]','MIRP[00101]','MDRP[00000]'):
  prog=push(2048)+['SCVTCI[ ]']+push(9)+['SRP0[ ]']+push(p)+(push(0) if opcode.startswith('MIRP') else [])+[opcode]
  for iup in (False,True):cases.append((f'p{p}-{opcode}-iup{int(iup)}',prog+(IUP if iup else [])))

CASES += cases

def generate(root=ROOT):
 order=['.notdef']+[f'c{i}' for i in range(len(CASES))];glyphs={};metrics={}
 for i,name in enumerate(order):
  pen=TTGlyphPen(None)
  if i:
   pen.moveTo(POINTS[0])
   for point in POINTS[1:]:pen.lineTo(point)
   pen.closePath()
  g=pen.glyph();g.program=Program();g.program.fromAssembly(['SVTCA[1]']+CASES[i-1][1] if i else [])
  glyphs[name]=g;metrics[name]=(351,8 if i else 0)
 fb=FontBuilder(4096,isTTF=True);fb.setupGlyphOrder(order);fb.setupCharacterMap({0xe000+i:f'c{i}' for i in range(len(CASES))});fb.setupGlyf(glyphs);fb.setupHorizontalMetrics(metrics);fb.setupHorizontalHeader(ascent=4096,descent=0)
 name='OOXML Compatible Width';fb.setupNameTable({'familyName':name,'styleName':'Regular','uniqueFontIdentifier':name+' 1.0','fullName':name,'psName':'OOXMLCompatibleWidth','version':'Version 1.000'});fb.setupOS2(sTypoAscender=4096,sTypoDescender=0,usWinAscent=4096,usWinDescent=0);fb.setupPost();fb.setupMaxp()
 font=fb.font;font['head'].flags=3;font['maxp'].maxStackElements=256;font['maxp'].maxStorage=64
 cvt=newTable('cvt ');cvt.values=array.array('h',[64]);font['cvt ']=cvt
 font.save(root/'cleartype-compatible-widths.ttf')
 return font
if __name__=='__main__':generate()
