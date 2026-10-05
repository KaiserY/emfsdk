"""Owned parent-program readbacks of current/original metric phantom points."""
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables.ttProgram import Program

root = Path(__file__).resolve().parent
font = TTFont(root / "natural-composite-programs.ttf")
order = list(font.getGlyphOrder())
controls = []
for index in [0, 1]:
    for original in [False, True]:
        name = f"phantom_{index}_{original}"
        pen = TTGlyphPen(font["glyf"])
        pen.addComponent("plain_511", (1, 0, 0, 1, 0, 0))
        glyph = pen.glyph()
        glyph.components[0].flags &= ~4
        glyph.program = Program()
        glyph.program.fromAssembly([
            "SVTCA[1]", "PUSHW[ ]", "5", str(4 + index),
            "GC[1]" if original else "GC[0]", "PUSHW[ ]", "1024", "MUL[ ]",
            "PUSHW[ ]", "1280", "ADD[ ]", "SCFS[ ]",
        ])
        glyph.recalcBounds(font["glyf"])
        font["glyf"][name] = glyph
        # A fractional nonzero bearing makes whole-pixel preparation observable.
        font["hmtx"][name] = (1000, glyph.xMin + 17)
        order.append(name)
        controls.append(name)
font.setGlyphOrder(order)
font["head"].flags &= ~2
for table in font["cmap"].tables:
    if table.isUnicode():
        table.cmap = {0xE000 + i: name for i, name in enumerate(controls)}
font.recalcTimestamp = False
font.save(root / "natural-composite-phantoms.ttf")
