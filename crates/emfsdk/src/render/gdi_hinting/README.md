# GDI ClearType interpreter adaptation

`hint/` is adapted from the private TrueType interpreter in **skrifa 0.46.2**,
`src/outline/glyf/hint/`, published by the Fontations project. It is used under
the bundled MIT license. Original opcode documentation and execution budgets
are retained. Upstream-only test helpers are omitted; the constructor accepts
an input structure instead of suppressing Clippy's argument-count warning.

Skrifa's public `Smooth(Lcd)` target suppresses horizontal point movement and
reports subpixel positioning. Classic Windows GDI uses different semantics
which cannot be selected through that API. This module supplies a private
adapter without modifying fonts, dependency sources, or the upstream checkout.

Differences from the upstream interpreter:

- `GETINFO` reports the observed Windows interpreter version (42), integer
  origin positioning, quality-dependent compatible widths, and the selected
  symmetric smoothing mode.
- Ordinary X movements run; delta instructions retain compatibility filtering.
- Projections with a nonzero X component use a 1/16-pixel grid during glyph
  execution, including diagonal projections and vertical freedom vectors.
  Control-value programs and pure Y projections retain the physical grid.
  Fine-grid CVT cut-in and minimum distance are divided by 16 and 2 respectively.
- Compatible unrounded MIRP and nonzero-distance MSIRP honor CVT cut-in.
- A guarded inline-delta function identified by its bytecode retains X-axis
  `SHPIX` moves under backward-compatible ClearType. Native font controls
  support this TypeMan-style exception; the unguarded helper remains filtered.
- Compatible-width rendering runs the complete monochrome program first to
  obtain its F26.6 advance, including late phantom-point instructions. Before
  the first horizontal IUP, the fine-grid outline follows that advance through
  relative/interpolated point dependencies and indirect stem-center constraints.
  A separate graph retains first-parent and first-stem relationships; ordinary
  IUP still interpolates untouched points afterward. No glyph names or font
  identities select these rules. An explicit interpolation between both
  horizontal bearings retains its advance anchor instead of being replaced
  by independent stem-center normalization.
- IUP multiplies and divides in one operation, retaining exact half-unit ties.
  When both anchors originally coincide, a point at that coordinate follows
  the next contour reference. Ordinary point movement remains enabled after
  IUP; delta-instruction filtering is handled separately.
- Initial CVT scaling rounds signed half-unit ties toward positive infinity.
  This differs from the symmetric fixed-point multiplication in the upstream
  scaler. The owned `cleartype-cvt-rounding.ttf` encodes 42 signed CVT values
  into glyph advances; DirectWrite GDI Natural and GDI qualities 3 and 6 agree
  on all 42 readbacks at 67 ppem. The private instance test retains those
  native values, including negative ties and their positive counterparts.

The public `render::GdiNaturalMetrics` adapter prepares and caches a separate
asymmetric, natural-width interpreter instance for integer pixel heights.
It reports whole-pixel horizontal advances without building a raster path.
Empty and uninstructed simple glyphs retain their metric phantom points;
instruction-free composites with one untransformed `USE_MY_METRICS` component
inherit that component's hinted advance, including nested references. This does
not change the existing raster target. Instructed composites with explicitly
offset children, unit axis reflections and their own metric phantoms execute
their parent program after their children. Native parent-program readbacks
establish a 1/16-pixel left phantom origin; its original right phantom preserves
the unrounded advance from that origin. Current horizontal phantoms then use
the fine grid, while vertical phantoms use the physical grid. The final width
rounds the bearing difference, rather than each final bearing independently.
General component scaling/rotation, point anchors, rounded/scaled offsets,
instructed inherited metrics, variations, CFF, font simulations and transformed
measurement retain explicit fallback.
Native DirectWrite controls cover 672 simple/empty advances and 112 inherited
composite advances at eight sizes; 24 additional checks retain explicit fallback
for unsupported composites. Fixtures and generators are project-owned:
`natural-advances` and `natural-composites` in `../testdata/`. A further 384
`natural-composite-programs` checks cover reflected/translated, nested and
multiple children; 32 `natural-composite-phantoms` checks distinguish current
and original parent bearings with a nonzero metric origin. These native
expectations require no installed fonts at test time. The system-font
matrix and native observation scripts remain outside the repository under
`/tmp/ooxmlsdk-bib-natural-api/` and
`/tmp/ooxmlsdk-bug65649/visual-page23/`.

References:

- [Microsoft ClearType instruction compatibility](https://learn.microsoft.com/en-us/typography/cleartype/truetypecleartype)
- [OpenType TrueType instructions](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions)
- [Fontations](https://github.com/googlefonts/fontations)
- [Microsoft compatible-width topology patent US6377262B1](https://patents.google.com/patent/US6377262B1/en)

The raster wrapper uses public Skrifa scaling for static, simple `glyf` outlines,
including head.flags FORCE_INTEGER_PPEM. Unsupported outlines (composites,
variable fonts, CFF), empty programs, and interpreter errors use the existing
Skrifa path. Font/control programs are prepared once per text request and size;
per-glyph CVT/storage changes remain isolated. This is not a complete Windows
rasterizer: compatible-width dependency tracking currently covers axis-aligned
glyph-zone links; twilight/composite/variation extensions and additional legacy
function-signature heuristics require their own native controls. The patent
describes the topology and width-preservation approach, while the exact rules
implemented here are established by independent native coordinate readbacks.

Native validation uses the project-owned `cleartype-instructions.ttf` and PNG
in `../testdata/`. Its 64 glyphs cover virtual-grid rounding, minimum distance,
MIAP/MIRP/MSIRP, SHP/SHPIX/DELTAP, physical prep rounding and vertical rounding.
Every native RGBA pixel is checked without requiring Windows at test time.
The project-owned `cleartype-compatible-{widths,stems}` fonts additionally cover
279 programs in both compatible (quality 5) and natural (quality 6) modes.
Their native expectations check all eight X coordinates, including forward
references, repeated IUP, late advance changes, indirect-distance flags,
independent/overlapping stems, cyclic links, and coincident IUP anchors.
`cleartype-projection` adds 83 programs in both modes for diagonal projection,
freedom-vector independence, rounding ties, cut-in/minimum distance and
bearing-anchored stems. All 724 coordinate comparisons run on every platform.
Generators use only project-owned outlines and bytecode; no installed font
data is required to regenerate these fixtures or run their tests.
