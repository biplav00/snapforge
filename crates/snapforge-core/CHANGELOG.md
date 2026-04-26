# Changelog

## [1.4.0](https://github.com/biplav00/snapforge/compare/v1.3.1...v1.4.0) (2026-04-26)


### Features

* implement SCK screenshot capture via SCScreenshotManager ([ad5d4f0](https://github.com/biplav00/snapforge/commit/ad5d4f03bb22383ee5be5c01ccbcc244e964c633))
* redesign history window with search, filters, and video support ([ef1f462](https://github.com/biplav00/snapforge/commit/ef1f462e82e87d78183bce3b23fb8b2b0bfbb20d))
* rewrite macos capture to SCK — display enum and permission ([c5e92fe](https://github.com/biplav00/snapforge/commit/c5e92fe9a4eac5c11c06d5076a8043209106513b))
* ripple click animation in recordings ([bfcbbef](https://github.com/biplav00/snapforge/commit/bfcbbefae58da8264c4bd6dd9346dee219eb2b3a))
* visualize mouse clicks in screen recordings ([83284dd](https://github.com/biplav00/snapforge/commit/83284dd8544b175f2b08cc3fa16f0eead9be5d41))


### Bug Fixes

* add CaptureContext for fast repeated captures during recording ([ba34a1a](https://github.com/biplav00/snapforge/commit/ba34a1a82a3b6cad2966168882ee5641bb0f9788))
* address code review findings across core, FFI, and Qt ([df15fe7](https://github.com/biplav00/snapforge/commit/df15fe7cfc68b2957514d2853fe894b65e76ac4e))
* address code review issues for SCK capture ([306503c](https://github.com/biplav00/snapforge/commit/306503c985fa7ba5816c820259fedccb7e3ec934))
* bundle ffmpeg as Tauri externalBin sidecar ([ca74760](https://github.com/biplav00/snapforge/commit/ca7476003cde4468b9f6ca51862619b21b34f8a0))
* crop recording frames to even dimensions for libx264 yuv420p ([3d48da4](https://github.com/biplav00/snapforge/commit/3d48da4a4e3a420e2d0075483ff9af5b827f7c06))
* edge-case hardening + overlay screenshot freshness ([6110a86](https://github.com/biplav00/snapforge/commit/6110a862a563513ece83a6f528aa4d278faf675f))
* move trigger_screenshot/recording off main thread to prevent SCK deadlock ([d309c32](https://github.com/biplav00/snapforge/commit/d309c32e561b0b6aebdbd14622e6c181d1ab0419))
* multiple recording fixes ([cf6a182](https://github.com/biplav00/snapforge/commit/cf6a1821abcfcffc424b861bead528710d9c3299))
* permission check fallback — try real capture if preflight fails ([71e801a](https://github.com/biplav00/snapforge/commit/71e801a65bb4e9665068b6969b632962553f1dfa))
* query display scale factor dynamically instead of hardcoding 2.0 ([169bdc4](https://github.com/biplav00/snapforge/commit/169bdc4255e8684669acf2632fde0eeae395295c))
* run SCK capture on background thread to avoid main thread deadlock ([07d5a42](https://github.com/biplav00/snapforge/commit/07d5a42a1e5a0448ae1629bf73bc5149d84539f1))
* use CGDisplayMode pixel dimensions for correct Retina resolution ([afc29dc](https://github.com/biplav00/snapforge/commit/afc29dc3b51035a328ad1294a6fc5e4c2fe0ba5b))


### Performance Improvements

* fast BGRA recording path with downscaling for macOS ([2facd56](https://github.com/biplav00/snapforge/commit/2facd56ffb90ccf27421f668ad782bd91693c619))
