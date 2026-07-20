# EMF SDK for Rust

[![crates.io](https://img.shields.io/crates/v/emfsdk.svg)](https://crates.io/crates/emfsdk)
[![docs.rs](https://docs.rs/emfsdk/badge.svg)](https://docs.rs/emfsdk)

`emfsdk` is a pure-Rust SDK for reading, inspecting, editing, and writing
Enhanced Metafile (EMF), Enhanced Metafile Format Plus (EMF+), and Windows
Metafile (WMF) data.

Known records and fields are exposed as Rust types. Unknown records, reserved
extensions, padding, and opaque binary payloads are preserved so an unmodified
metafile can be written without losing data.

## Quick Start

```bash
cargo add emfsdk
```

```rust
use emfsdk::Metafile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::fs::read("input.emf")?;
    let metafile = Metafile::from_bytes(&input)?;

    println!("format: {:?}", metafile.format());

    let output = metafile.to_bytes()?;
    std::fs::write("output.emf", output)?;
    Ok(())
}
```

Use `EmfMetafile`, `WmfMetafile`, and each record's `parse_data` method when
you need format-specific typed access. Owned metafiles and records implement
`SdkWrite`; top-level `write_to` methods accept any `std::io::Write` sink and do
not allocate an intermediate metafile buffer. Low-level `Writer` tracks its own
position without requiring `Seek`; use `Writer::with_position` when serialization
starts at a nonzero logical offset.

For read-only framing, `MetafileRef`, `EmfMetafileRef`, `WmfMetafileRef`, and
`EmfPlusStreamRef` validate the complete record stream and borrow record data
directly from the input buffer. Their exact-size iterators do not allocate.
Record-level `parse_data` remains eager. Call `into_owned` explicitly before
editing or writing; borrowed views do not provide a raw pass-through writer.
The owned `EmfPlusStream` retains both records and producer trailing bytes;
`from_bytes_exact` is available when trailing bytes must be rejected.

## Compatible and Strict Parsing

`Metafile::from_bytes` is compatibility-first. It preserves bounded records
that are structurally readable even when a producer violates a field rule.
Compatibility is explicit and inspectable:

```rust,no_run
use emfsdk::Metafile;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let input = std::fs::read("input.wmf")?;
let metafile = Metafile::from_bytes(&input)?;
for diagnostic in metafile.compatibility_diagnostics() {
    eprintln!("{diagnostic:?}");
}
# Ok(())
# }
```

Use `Metafile::from_bytes_strict` or `validate_strict` when every known record
must satisfy the Microsoft specification and reproduce exactly through its
typed parser and writer. Unknown record types remain an intentional raw
fallback; malformed known records are reported instead of silently treated as
typed.

## Features

- `default`: parsing and writing only, with no rendering dependencies
- `render`: enables raster rendering with `image`, `fontdb`, and `ttf-parser`

```bash
cargo add emfsdk --features render
```

The minimum supported Rust version is 1.88. The workspace uses the Rust 2024
edition.

## Format Coverage

The implementation follows the Microsoft Open Specifications:

- [[MS-EMF]: Enhanced Metafile Format](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emf/e0137630-f3ad-492c-bde9-e68866e255ba)
- [[MS-EMFPLUS]: Enhanced Metafile Format Plus Extensions](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/229f98d8-c19a-464e-80cc-2cb96aba1d71)
- [[MS-WMF]: Windows Metafile Format](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/4813e7fd-52d0-4f42-965f-228c8b7488d2)

LibreOffice and Apache POI are used as behavioral references. Corpus and
round-trip tests live in
[`ooxmlsdk-test-suite`](https://github.com/KaiserY/ooxmlsdk-test-suite).

The standalone corpus covers 950,508 EMF records, 22,824 WMF records, and
14,274 nested EMF+ records. Typed parse/write rebuilds 987,591 of 987,606
records exactly (99.998%); the remaining 15 structurally malformed records are
counted raw compatibility fallbacks, with no unknown record types. The legacy
Office lane covers another 107,584 records, of which 107,446 rebuild through
typed parse/write (99.872%) and 138 use counted raw fallback. Tests ratchet
these counts and require exact whole-file bytes plus exact typed parse/write
bytes for every typed record.

## Project Status

The 0.2 line remains a pre-1.0 SDK. Byte-preserving parsing, typed field
coverage, and reducing the remaining structurally malformed fallbacks are the
primary focus. The public API can change before 1.0.

The optional renderer is suitable for compatibility and preview workflows, but
does not promise pixel-identical output with Windows GDI, GDI+, or
LibreOffice.

## Changelog

See [CHANGELOG.md](./CHANGELOG.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))
- MIT License ([LICENSE-MIT](./LICENSE-MIT))

at your option.
