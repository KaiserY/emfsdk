"""Generate the project-owned indirect-distance controls, without system fonts."""
import array
import json
from pathlib import Path

from fontTools.ttLib import newTable
from fontTools.ttLib.tables.ttProgram import Program
import generate_compatible_widths as widths

root = Path(__file__).resolve().parent
rows = json.loads((root / "cleartype-compatible-stems.json").read_text())
widths.POINTS = [(98,320),(226,320),(290,0),(98,0),(98,64),(98,128),(98,192),(98,256)]
widths.CASES = [(row["name"], row["assembly"]) for row in rows]
# Build in memory: preserve the independent compatible-width fixture.
from tempfile import TemporaryDirectory
with TemporaryDirectory() as temporary:
    font = widths.generate(Path(temporary))
for name in font.getGlyphOrder():
    font['hmtx'][name] = (351, 98 if name != '.notdef' else 0)
cvt = newTable('cvt ')
cvt.values = array.array('h', [64,84,0])
font['cvt '] = cvt
prep = newTable('prep')
prep.program = Program()
prep.program.fromAssembly(widths.push(4096) + ['SCVTCI[ ]'])
font['prep'] = prep
font.save(root / 'cleartype-compatible-stems.ttf')
