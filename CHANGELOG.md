# Changelog

All notable changes to this project are documented in this file.

## 0.1.0

### Added

- Added typed parsing and writing for EMF, EMF+, and WMF metafiles and their
  known record types.
- Added byte-preserving round trips for unknown records, extensions, padding,
  strings, bitmap payloads, and other opaque data.
- Added typed EMF+ object, brush, pen, path, region, image, font, string format,
  image effect, and terminal-server data.
- Added DIB header, color table, bitmap payload, and embedded image support.
- Added optional raster rendering behind the `render` feature.
- Added derive support for fixed-layout SDK objects and numeric enums.
- Added compatibility-first metafile parsing with lightweight structured
  diagnostics and explicit strict validation.

### Testing

- Added specification-focused unit tests for record layouts, validation,
  strings, bitmaps, and read/write behavior.
- Validated standalone and Office-embedded metafiles against the external
  LibreOffice, Apache POI, ClosedXML, and libemf2svg corpora in
  `ooxmlsdk-test-suite`.
- Ratcheted record totals, typed compatibility fallbacks, unknown records, and
  whole-file byte equality so parse failures cannot be silently skipped.

### Known limitations

- Some real-world producer records still require raw compatibility
  preservation instead of typed reconstruction.
- The optional renderer does not promise pixel-identical Windows GDI/GDI+ or
  LibreOffice output.
