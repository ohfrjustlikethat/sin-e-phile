# source-protocol

Part of [sin-e-phile](https://github.com/ohfrjustlikethat/sin-e-phile), but
**independently useful and independently licensed** (ADR-0017).

The versioned `SourceBackend` protocol and the declarative addon manifest
format: types, JSON Schema, and validation. Everything a third party needs to
implement a source backend.

Declarative by design — a source is described as data, never shipped as
executable code (ADR-0006). Built in Phase 6.

## Licence

Dual-licensed under **MIT OR Apache-2.0**, at your option — the Rust convention.

The parent application is GPL-3.0, because it links libmpv and FFmpeg. **This crate
links neither**, so it carries the permissive licence that makes it actually usable
by the ecosystem. See [ADR-0017](../../docs/adr/0017-dual-license-extracted-crates.md).
