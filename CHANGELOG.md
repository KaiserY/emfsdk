# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Changed

- Made compatible parsing preserve complete producer-specific field values and
  reserved bits while keeping Microsoft specification checks in explicit
  `validate_strict` paths.
- Added typed, byte-exact handling for short WMF font names, short EMF color
  space names, EMF+ object deviations, and other common corpus variants.
- Corrected the EMF+ continued-object wire layout and the record-relative
  `offData` base used by `EMR_COMMENT_MULTIFORMATS`.
- Added explicit compatibility fields for trailing EMF+ comment bytes and
  producer-specific WMF EOF, pen, and flood-fill data; strict validation
  rejects these extensions.
- Preserved compatible DIB, raster-operation, and text producer variants while
  keeping specification validation opt-in through strict mode.
- Made strict metafile validation stop at the first failure instead of building
  the complete diagnostics collection.
- Added fully framed and structurally validated borrowed EMF, EMF+, and WMF
  record streams with allocation-free iteration and explicit owned
  materialization.
- Changed typed `Unknown` variants to carry borrowed record views, avoiding an
  owned outer record requirement during typed inspection.
- Switched WMF rendering to borrowed record framing so it no longer clones the
  complete record payload set before typed replay.
- Renamed consuming borrowed conversions to `into_owned`, added uniform
  record-level `rebuild_typed` methods that preserve wire metadata, and exposed
  streaming `write_to` support through `SdkWrite` on owned metafiles and
  records.
- Added an owned `EmfPlusStream` so conversion from `EmfPlusStreamRef` retains
  trailing producer bytes; exact stream consumption is now an explicit
  `from_bytes_exact` choice.
- Removed the `Seek` requirement from `Writer` and `SdkWrite`. The writer now
  tracks successful writes internally, top-level streams accept ordinary
  `Write` sinks, and preallocated byte serialization writes directly into
  `Vec<u8>`.
- Moved the fixed WMF headers and EMF+ image-attributes object layout onto
  `SdkObject`, keeping their compatibility validation and public convenience
  methods while removing duplicate read, write, and size implementations.

### Testing

- Reduced standalone raw compatibility fallbacks from 31,922 to 15 across
  987,606 records, including two malformed nested EMF+ fallbacks and zero
  unknown records (99.998% typed).
- Reduced Office-embedded raw fallbacks from 3,955 to 138 across 107,584
  records (99.872% typed) while preserving exact whole-file and typed record
  bytes.
- Added a corpus coverage profiler and tightened both standalone and embedded
  Office coverage ratchets.

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
