"""Generate owned projection and bearing-interpolation controls from assembly."""
import array
import json
from pathlib import Path
from tempfile import TemporaryDirectory

from fontTools.ttLib.tables._g_l_y_f import GlyphCoordinates
from fontTools.ttLib.tables.ttProgram import Program
import generate_compatible_widths as widths

root = Path(__file__).resolve().parent
rows = json.loads((root / "cleartype-projection.json").read_text())
widths.POINTS = rows[0]["points"]
widths.CASES = [(row["name"], []) for row in rows]
with TemporaryDirectory() as temporary:
    font = widths.generate(Path(temporary))
font["cvt "].values = array.array("h", [0, 0, 0])
for index, row in enumerate(rows):
    name = f"c{index}"
    glyph = font["glyf"][name]
    glyph.coordinates = GlyphCoordinates(row["points"])
    # Reproduce the independent native fonts' CVT and cut-in per glyph.
    assembly = []
    for cvt_index, value in enumerate(row["cvt"]):
        assembly += widths.push(cvt_index, value) + ["WCVTP[ ]"]
    assembly += widths.push(row["cutin"]) + ["SCVTCI[ ]"]
    glyph.program = Program()
    glyph.program.fromAssembly(assembly + row["assembly"])
    font["hmtx"][name] = (351, 98)
font.save(root / "cleartype-projection.ttf")
