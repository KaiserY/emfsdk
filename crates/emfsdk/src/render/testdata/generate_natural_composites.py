"""Project-owned USE_MY_METRICS controls; run generate_natural_advances.py first."""
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables.ttProgram import Program
root=Path(__file__).resolve().parent
f=TTFont(root/'natural-advances.ttf');rows=[];order=list(f.getGlyphOrder())
for base in ['plain_511','set_advance_1706']:
 for shift in [-128,0,63]:
  for extra in [False,True]:
   name=f'alias_{base}_{shift}_{extra}';pen=TTGlyphPen(f['glyf']);pen.addComponent(base,(1,0,0,1,shift,32))
   if extra:pen.addComponent('plain_256',(1,0,0,1,0,128))
   g=pen.glyph();g.components[0].flags|=0x200;f['glyf'][name]=g;f['hmtx'][name]=(1000,0);order.append(name);rows.append({'name':name,'base':base})
for base in [rows[0]['name'],rows[-1]['name']]:
 name='nested_'+base;pen=TTGlyphPen(f['glyf']);pen.addComponent(base,(1,0,0,1,0,0));g=pen.glyph();g.components[0].flags|=0x200;f['glyf'][name]=g;f['hmtx'][name]=(1500,0);order.append(name);rows.append({'name':name,'base':base})
for kind in ['no_metrics','scaled','instructions']:
 name=kind;pen=TTGlyphPen(f['glyf']);pen.addComponent('set_advance_1706',(.5,0,0,1,0,0)if kind=='scaled'else(1,0,0,1,0,0));g=pen.glyph()
 if kind!='no_metrics':g.components[0].flags|=0x200
 if kind=='instructions':g.program=Program();g.program.fromAssembly(['SVTCA[1]'])
 f['glyf'][name]=g;f['hmtx'][name]=(1000,0);order.append(name);rows.append({'name':name,'unsupported':True})
f.setGlyphOrder(order)
for t in f['cmap'].tables:
 if t.isUnicode():t.cmap={0xe000+i:r['name']for i,r in enumerate(rows)}
f.recalcTimestamp = False
f['head'].modified = 3873440069  # Fixed native-control fixture timestamp.
f.save(root/'natural-composites.ttf')
