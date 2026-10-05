"""Owned composite instructions and transforms; no installed font data.

Requires generate_natural_advances.py. Native expected advances are recorded
separately; this generator does not derive them from the SDK.
"""
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables.ttProgram import Program

root = Path(__file__).resolve().parent
font = TTFont(root / "natural-advances.ttf")
order = list(font.getGlyphOrder())
pen = TTGlyphPen(None)
pen.moveTo((0, 0))
pen.lineTo((64, 0))
pen.lineTo((64, 64))
pen.lineTo((0, 64))
pen.closePath()
glyph = pen.glyph()
glyph.program = Program()
glyph.program.fromAssembly(["SVTCA[1]", "PUSHW[ ]", "0", "640", "SCFS[ ]"])
font["glyf"]["move_origin"] = glyph
font["hmtx"]["move_origin"] = (511, 0)
order.append("move_origin")
controls = []


def add(name, children, parent_program):
    pen = TTGlyphPen(font["glyf"])
    for child, transform in children:
        pen.addComponent(child, transform)
    glyph = pen.glyph()
    # Offset rounding/inherited transformed metrics have separate contracts.
    for component in glyph.components:
        component.flags &= ~4
    glyph.program = Program()
    count = sum(len(font["glyf"][child].getCoordinates(font["glyf"])[0]) for child, _ in children)
    code = ["SVTCA[1]"]
    if parent_program == "coordinate":
        # A child instruction moves point0 before the parent's GC reads it.
        code += ["PUSHW[ ]", str(count + 1), "0", "GC[0]", "PUSHW[ ]", "1280", "ADD[ ]", "SCFS[ ]"]
    glyph.program.fromAssembly(code)
    glyph.recalcBounds(font["glyf"])
    font["glyf"][name] = glyph
    font["hmtx"][name] = (1000, glyph.xMin)
    order.append(name)
    controls.append(name)


for child in ["plain_511", "set_advance_1706", "move_origin"]:
    for xx in [-1, 1]:
        for offset in [-128, 0, 63]:
            for program in ["noop", "coordinate"]:
                add(f"composite_{len(controls)}", [(child, (xx, 0, 0, 1, offset, 32))], program)
for base in controls[-6:].copy():
    add(f"nested_{len(controls)}", [(base, (1, 0, 0, 1, 31, -63))], "coordinate")
for base in ["plain_511", "move_origin"]:
    for offset in [-63, 0, 128]:
        add(f"multi_{len(controls)}", [(base, (-1, 0, 0, 1, offset, 0)), ("plain_256", (1, 0, 0, 1, 0, 128))], "coordinate")
font.setGlyphOrder(order)
for table in font["cmap"].tables:
    if table.isUnicode():
        table.cmap = {0xE000 + i: name for i, name in enumerate(controls)}
font.recalcTimestamp = False
font["head"].modified = 3873440069
font.save(root / "natural-composite-programs.ttf")
