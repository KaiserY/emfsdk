"""Project-owned GDI Natural metrics controls; no imported outlines or bytecode."""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables.ttProgram import Program
root=Path(__file__).resolve().parent
widths=[0,1,10,255,256,257,500,511,512,513,683,700,768,1000,1023,1024,1025,1536,1706,2048,2500]
glyphs={}; metrics={}; rows=[]
for kind in ['empty','plain','noop','set_advance']:
 for width in widths:
  name=f'{kind}_{width}'; pen=TTGlyphPen(None)
  if kind!='empty':
   pen.moveTo((0,0));pen.lineTo((64,0));pen.lineTo((64,64));pen.lineTo((0,64));pen.closePath()
  g=pen.glyph()
  if kind in ['noop','set_advance']:
   g.program=Program();g.program.fromAssembly(['SVTCA[1]']+(['PUSHW[ ]','5',str(width),'SCFS[ ]'] if kind=='set_advance' else []))
  glyphs[name]=g;metrics[name]=(width,0);rows.append({'kind':kind,'width':width,'name':name})
pen=TTGlyphPen(None);glyphs['.notdef']=pen.glyph();metrics['.notdef']=(0,0)
order=['.notdef']+[r['name'] for r in rows]
fb=FontBuilder(2048,isTTF=True);fb.setupGlyphOrder(order);fb.setupCharacterMap({0xe000+i:r['name'] for i,r in enumerate(rows)})
fb.setupGlyf(glyphs);fb.setupHorizontalMetrics(metrics);fb.setupHorizontalHeader(ascent=2048,descent=0)
name='OOXML Natural Advances';fb.setupNameTable({'familyName':name,'styleName':'Regular','uniqueFontIdentifier':name+' 1.0','fullName':name,'psName':'OOXMLNaturalAdvances','version':'Version 1.000'})
fb.setupOS2(sTypoAscender=2048,sTypoDescender=0,usWinAscent=2048,usWinDescent=0);fb.setupPost();fb.setupMaxp();fb.font['head'].flags=19;fb.font['head'].created=fb.font['head'].modified=3800000000;fb.font['maxp'].maxStackElements=32
fb.font.save(root/'natural-advances.ttf')
