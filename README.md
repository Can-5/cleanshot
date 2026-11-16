# CleanShot

**Screenshot annotation model and archive logic for Orbit — no capture engine.**

> Orbit companion module — phase 2/4 of the [Orbit ecosystem](https://github.com/Can-5/orbit).
> Standalone crate: usable without Orbit, without other modules, and without network.

## Implemented now

- Region, ImageSize, Annotation enum (Arrow/Rectangle/Text/Blur/Highlight) with strict validation
- CaptureMeta and deterministic archive_filename(orbit-shot-YYYYMMDD-HHMMSS-NNN.png)

## Not implemented yet (explicitly out of scope for this crate)

- No screen capture — that is the Orbit host, gated by ScreenCapture permission
- No image encoding/writing — the host owns pixels and files

## Build & test

```sh
cargo test  # 8 tests
```

Dependencies: serde, thiserror  # serde_json (dev). No cross-repo path dependencies, no Git dependencies, no downloads at build time beyond crates.io.

## Privacy

Annotations never leave the device. Blur annotations are stored as metadata until the host renders them; there is no network code.

## License

MIT © The Orbit contributors. See [LICENSE](./LICENSE).
