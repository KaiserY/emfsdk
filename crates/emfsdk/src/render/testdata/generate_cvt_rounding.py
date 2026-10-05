"""Project-owned CVT signed half-tie controls (no imported glyphs/programs)."""
from pathlib import Path
from array import array
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import newTable
from fontTools.ttLib.tables.ttProgram import Program

ROOT = Path(__file__).resolve().parent
VALUES = [i * 16 for i in range(-21, 21)]

def generate(root=ROOT):
    order = ['.notdef'] + [f'c{i}' for i in range(len(VALUES))]
    glyphs = {}
    for i, name in enumerate(order):
        pen = TTGlyphPen(None)
        pen.moveTo((0, 0)); pen.lineTo((64, 0))
        pen.lineTo((64, 64)); pen.lineTo((0, 64)); pen.closePath()
        glyph = pen.glyph()
        glyph.program = Program()
        # Encode signed CVT as a positive whole-pixel advance, preserving all
        # F26.6 bits through GetGdiCompatibleGlyphMetrics. At 67 ppem decode
        # round(advanceWidth * 67 / 4096) - 2048.
        if i:
            glyph.program.fromAssembly(
                ['SVTCA[1]', 'PUSHW[ ]', '5', str(i - 1), 'RCVT[ ]',
                 'PUSHW[ ]', '2048', 'ADD[ ]']
                + ['DUP[ ]', 'ADD[ ]'] * 6 + ['SCFS[ ]'])
        glyphs[name] = glyph
    fb = FontBuilder(4096, isTTF=True)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap({0xe000+i: name for i, name in enumerate(order[1:])})
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics({name: (512, 0) for name in order})
    fb.setupHorizontalHeader(ascent=4096, descent=0)
    name = 'OOXML CVT Rounding'
    fb.setupNameTable({'familyName': name, 'styleName': 'Regular',
        'uniqueFontIdentifier': name+' 1.0', 'fullName': name,
        'psName': 'OOXMLCVTRounding', 'version': 'Version 1.000'})
    fb.setupOS2(sTypoAscender=4096, sTypoDescender=0, usWinAscent=4096, usWinDescent=0)
    fb.setupPost(); fb.setupMaxp()
    font = fb.font
    font['head'].flags = 19
    font['head'].created = font['head'].modified = 3800000000
    font['maxp'].maxStackElements = 256
    font['cvt '] = newTable('cvt ')
    font['cvt '].values = array('h', VALUES)
    font.save(root/'cleartype-cvt-rounding.ttf')

if __name__ == '__main__':
    generate()
