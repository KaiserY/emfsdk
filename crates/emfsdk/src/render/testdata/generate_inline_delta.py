"""Generate project-owned GDI SHPIX function-signature controls.

Requires fontTools. Native expectations are independently captured and must not
be regenerated from implementation output.
"""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables.ttProgram import Program
from fontTools.ttLib import newTable
import json
D=Path(__file__).resolve().parent
push=lambda *a:['PUSHW[ ]'if any(x>255 or x<0 for x in a)else'PUSHB[ ]',*map(str,a)]
rangebody=['MPPEM[ ]','GTEQ[ ]','SWAP[ ]','MPPEM[ ]','LTEQ[ ]','AND[ ]','IF[ ]','SHPIX[ ]','ELSE[ ]','POP[ ]','POP[ ]','EIF[ ]']
pointbody=['MPPEM[ ]','EQ[ ]','IF[ ]','SHPIX[ ]','ELSE[ ]','POP[ ]','POP[ ]','EIF[ ]']
def guard(body,pops):return push(2)+['RS[ ]','EQ[ ]','IF[ ]']+body+['ELSE[ ]']+['POP[ ]']*pops+['EIF[ ]']
functions=[rangebody,guard(rangebody,4),pointbody,push(0)+['POP[ ]']+pointbody,guard(push(2)+['CALL[ ]'],3),guard(push(3)+['CALL[ ]'],3),push(0)+['POP[ ]']+rangebody,guard(push(6)+['CALL[ ]'],4),['SHPIX[ ]'],guard(pointbody,3)]
fontprogram=[]
for idx,body in enumerate(functions):fontprogram+=push(idx)+['FDEF[ ]']+body+['ENDF[ ]']
order=['.notdef'];glyphs={};metrics={};cases=[];pen=TTGlyphPen(None);glyphs['.notdef']=pen.glyph();metrics['.notdef']=(512,0)
for touch_axis in ('x','y'):
 for helper in range(len(functions)):
  for touched in (False,True):
   for after_iup in (False,True):
    idx=len(cases);name=f'g{idx}';order.append(name);pen=TTGlyphPen(None);pen.moveTo((64,512));pen.lineTo((320,512));pen.lineTo((320,0));pen.lineTo((64,0));pen.closePath();g=pen.glyph()
    prog=['SVTCA[1]']
    if touched:prog+=['SVTCA[0]' if touch_axis=='y' else 'SVTCA[1]']+push(0)+['MDAP[0]','SVTCA[1]']
    if after_iup:prog+=['IUP[1]','IUP[0]']
    args=[0,128]
    if helper in(0,1,6,7):args += [64,64]
    elif helper!=8:args += [64]
    if helper in(1,4,5,7,9):args +=[6]
    prog+=push(*args,helper)+['CALL[ ]','IUP[1]','IUP[0]']
    # Read the final x of point0 to an integer vertical marker. Reset all
    # zones/loop/projection, then replace the drawing, after the queried hints.
    prog+=['CLEAR[ ]']+push(1)+['SZPS[ ]']+push(1)+['SLOOP[ ]','SVTCA[1]']+push(0)+['GC[0]']+push(63)+['SWAP[ ]','WS[ ]']
    for k in range(4):prog+=push(k,256 if k in(1,2)else 0)+['SCFS[ ]']
    prog+=['SVTCA[0]']
    for k in range(4):
     prog+=push(k,63)+['RS[ ]']
     if k in(0,1):prog+=push(64)+['ADD[ ]']
     prog+=['SCFS[ ]']
    g.program=Program();g.program.fromAssembly(prog);glyphs[name]=g;metrics[name]=(512,64);cases.append(dict(glyph=name,index=idx,helper=helper,touched=touched,touch_axis=touch_axis,after_iup=after_iup))
fb=FontBuilder(4096,isTTF=True);fb.setupGlyphOrder(order);fb.setupCharacterMap({0xe000+c['index']:c['glyph']for c in cases});fb.setupGlyf(glyphs);fb.setupHorizontalMetrics(metrics);fb.setupHorizontalHeader(ascent=4096,descent=0);family='SDK Inline Delta Controls';fb.setupNameTable({'familyName':family,'styleName':'Regular','uniqueFontIdentifier':family,'fullName':family,'psName':'SDKInlineDeltaControls','version':'Version 1.000'});fb.setupOS2(sTypoAscender=4096,sTypoDescender=0,usWinAscent=4096,usWinDescent=0);fb.setupPost();fb.setupMaxp();f=fb.font;f['head'].flags=3;f['maxp'].maxStackElements=64;f['maxp'].maxStorage=64;f['maxp'].maxFunctionDefs=10
for table,asm in [('fpgm',fontprogram),('prep',push(2,6)+['WS[ ]'])]:
 t=newTable(table);t.program=Program();t.program.fromAssembly(asm);f[table]=t
f.save(D/'cleartype-inline-delta.ttf');(D/'cleartype-inline-delta.json').write_text(json.dumps(cases,indent=2)+'\n')
