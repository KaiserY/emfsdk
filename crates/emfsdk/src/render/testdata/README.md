# Synthetic font for private rendering tests

`cleartype-rectangle.ttf` is generated for this project, with no imported font
data. It has 4096 units per em, an empty `.notdef`, and one glyph mapped to A:
clockwise points `(98,320), (226,320), (226,0), (98,0)`, advance 512, left
bearing 98. Ascender is 4096 and descender is zero. It contains no hinting
programs. `head.flags` is 3; the test toggles FORCE_INTEGER_PPEM in memory.

The expected coverage comes from native GDI ExtTextOutW at 64 ppem with
ClearType quality, independently varied X transforms, and identical glyphs
with and without FORCE_INTEGER_PPEM. This isolates the physical font-size
policy from the later six-sample raster grid.

The two-axis rectangle test constructs the same outline directly, moving it
through an 8-by-8 grid of eighth-pixel offsets. Its pinned coverage table comes
from native GDI quality 5 on a white DIB. Independent diagonal and vertical
64-phase probes establish the symmetric five-row weights `1,2,3,2,1` and
joint quantization to six coverage levels. Version-0 and all sixteen version-1
`gasp` flag combinations separately establish the smoothing-mode selection.

`cleartype-instructions.ttf` is also wholly project-generated. Its 64 simple
rectangle glyphs map U+0021 through U+0060. Sixteen instruction cases each use
four initial phases, with 4096 units per em and an 8-pixel advance at 64 ppem.
`cleartype-instructions.png` is the independent Windows GDI quality-5 output
at 64 ppem, drawn with ExtTextOutW on a transparent GDI+ bitmap. The baseline
positions are `(10 + (index % 32) * 8, 10 + (index / 32) * 12)`.
The PNG contains only these project-owned shapes. The native GDI+ scratch
background is RGB `(13,11,12)`, and untouched pixels become transparent.

`cleartype-getinfo.ttf` and `cleartype-version.ttf` use the same project-owned
rectangles and sheet geometry. Their bytecode changes the top edge from 5 to
7 pixels when a query is true. The version sheet tests GETINFO selector 1 for
equality with every integer from 32 through 95. The info sheet repeats thirteen
queries: version 35 through 40, then nonzero results for selectors 64, 128, 256,
512, 1024, 2048, 4096. Native quality 5/6 PNGs establish version 42, the
quality-dependent compatible-width flag, and the remaining rendering flags.

`cleartype-compatible-widths.ttf` contains 75 project-owned eight-point glyph
programs; `cleartype-compatible-stems.ttf` contains 204. Their generators use
FontTools and require no system-font inputs. The stem programs are also retained
as readable assembly in `cleartype-compatible-stems.json`.

The accompanying `.txt` files contain native Windows GDI observations at
64 ppem for qualities 5 and 6: glyph index, quality, case name, then the eight
hinted X coordinates in F26.6 units. These were measured by appending GC[0]
coordinate reads to private experimental copies and encoding each coordinate
as ten binary rectangle markers. Every sheet includes a known-value calibration.
The runtime tests execute the original programs and compare their output
coordinates directly; they do not need Windows, the marker copies, or Office.

`cleartype-projection.ttf` adds 83 owned eight-point programs: 33 projection /
freedom-vector / CVT combinations, 33 rounding-tie variants, 15 cut-in/minimum
distance controls, and two stems anchored between both horizontal bearings.
Its JSON contains the original geometry, CVT and assembly; `generate_projection.py`
builds the fixture without system fonts. The `.txt` records all eight native
coordinates for qualities 5 and 6, using the same independent marker readback.
