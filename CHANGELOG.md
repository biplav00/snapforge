# Changelog

All notable changes to Snapforge are documented here. This file is updated
by `packaging/ci/update-changelog.sh` during releases; generated sections
should not be edited by hand.

## [1.4.3] - 2026-11-10

### Maintenance

- **ci:** bump actions to Node 24 releases and pin Ubuntu 24.04 ([5d557f8](https://github.com/biplav00/snapforge/commit/5d557f8e0af5dda116a47dea9dea7bc68b95a179))
- **deps:** bump the all-cargo group across 1 directory with 9 updates ([064375e](https://github.com/biplav00/snapforge/commit/064375ecd561f00b2c58f99add7f223e6800bd33))

## [1.4.2] - 2026-11-10

### Maintenance

- exclude promotion PRs at the event level ([4e7161f](https://github.com/biplav00/snapforge/commit/4e7161fc7ee0cf38a998a5d460da281a4e84551f))
- **release:** stop attaching third-party sources ([17a3669](https://github.com/biplav00/snapforge/commit/17a3669b921d99bb6c1a49fd68f3fc44212d320c))
- skip redundant runs on promotion pull requests ([a4b1414](https://github.com/biplav00/snapforge/commit/a4b141464b6f1b575b6d630f3461c108a7587e93))

## [1.4.1] - 2026-11-10

### Maintenance

- **release:** stop attaching a project source zip ([292da56](https://github.com/biplav00/snapforge/commit/292da5684a08cfe934c8da7ebfa7090b572defb2))

## [1.4.0] - 2026-11-10

### Features

- soften recording UI with stop-only tray and hairline border ([a5bda5a](https://github.com/biplav00/snapforge/commit/a5bda5a01234b115ec247ca61b18bfcc30af4d16))

### Fixes

- route tray banners through the visible menu-bar item ([b4ef7f3](https://github.com/biplav00/snapforge/commit/b4ef7f3517e9caeff77f23496189c6b4094008d3))
- show recording region border and enlarge tray timer ([04cfa89](https://github.com/biplav00/snapforge/commit/04cfa899a83c0ce2d0f345eefbd91e32ca269c78))

### Maintenance

- use Homebrew qtbase instead of the full qt meta-formula ([ee48778](https://github.com/biplav00/snapforge/commit/ee48778367cb42fb0d99ead756a18f4bf8452f88))
- parallelize policy checks and trim macOS build time ([f7f8a55](https://github.com/biplav00/snapforge/commit/f7f8a5532cfde2f3ef7577e2961c7b6c537677f3))
- move macOS runners from retired macos-14 to macos-15 ([f7550d8](https://github.com/biplav00/snapforge/commit/f7550d809f02fb21b72bc426b05c10b975e4e34a))
- **release:** add production branch, promotion, versioning, and changelog ([89f4acb](https://github.com/biplav00/snapforge/commit/89f4acb859ae375328e1f2ef44d8a24f47617b52))
- remove design mock files ([75b08c9](https://github.com/biplav00/snapforge/commit/75b08c9e9b74b2fc9730302335de3a13c7827929))

## [1.3.21] - 2026-11-05

### Fixes

- recover hotkeys after wake and polish overlay interactions ([cd7b4fb](https://github.com/biplav00/snapforge/commit/cd7b4fbc0a616ecc5edae9fb6e126763b9cab688))
- full-codebase review — crash/data-loss fixes, recording robustness, SCStream capture, CI-gated releases ([#54](https://github.com/biplav00/snapforge/pull/54))

### Maintenance

- **deps:** bump crossbeam-epoch to 0.9.20 ([6b2950a](https://github.com/biplav00/snapforge/commit/6b2950a9f7c254ffa23ea3c1f75c0114f3381803))

## [1.3.20] - 2026-07-07

### Fixes

- whole-app review — durability, teardown safety, UI responsiveness ([#52](https://github.com/biplav00/snapforge/pull/52))

## [1.3.19] - 2026-07-06

### Maintenance

- remove over-engineering flagged by ponytail audit ([#51](https://github.com/biplav00/snapforge/pull/51))

## [1.3.18] - 2026-06-27

### Fixes

- **ci:** anchor release version on git tags (#50) [skip release] ([39d1fac](https://github.com/biplav00/snapforge/commit/39d1fac86f6a32233d2ff5188978af723aaeb9bc))

### Other Changes

- ship recording cursor/clicks/region fixes ([ce65c78](https://github.com/biplav00/snapforge/commit/ce65c785a8e2b3eeb811d9419de5084340721f97))
- Architecture review: FFI seam + serde schema, plus recording cursor/clicks/region fixes ([#49](https://github.com/biplav00/snapforge/pull/49))
- Architecture review: deepen the FFI seam + 4 supporting refactors ([#48](https://github.com/biplav00/snapforge/pull/48))

## [1.3.17] - 2026-06-12

### Fixes

- **overlay:** make region-select crosshair deterministic on macOS ([3199881](https://github.com/biplav00/snapforge/commit/3199881203a7ec299c3cc2d7992d9607d6682337))

## [1.3.16] - 2026-06-11

### Maintenance

- **icon:** match app icon to the tray aperture mark ([#46](https://github.com/biplav00/snapforge/pull/46))

## [1.3.15] - 2026-06-11

### Fixes

- multi-display recording bug-hunt — FFI safety, teardown deadlocks, overlay re-anchoring ([#45](https://github.com/biplav00/snapforge/pull/45))

## [1.3.14] - 2026-06-02

### Fixes

- **overlay:** render region-select crosshair on activation ([28dcd79](https://github.com/biplav00/snapforge/commit/28dcd79c817b30ef064c0a92421d9bf8e1f01120))

### Performance

- **annotation:** cache committed-annotation layer ([ecf6621](https://github.com/biplav00/snapforge/commit/ecf66217b998ce2b32271dda25100c49e85bd0a4))
- **ffi:** cheap pointer hasher for the buffer/handle registries ([52e8e76](https://github.com/biplav00/snapforge/commit/52e8e766240935ae43ee48272ed6f345f88228dc))
- **capture:** parallelize RGBA swap, Arc the display-id cache ([b15fa7c](https://github.com/biplav00/snapforge/commit/b15fa7c9cbfc6013306736081987f0bfb1c4a5a5))
- **encode:** cut JPEG pre-encode copy, parallelize pixel work ([d77d3d6](https://github.com/biplav00/snapforge/commit/d77d3d6a1fc026a30a7d8fd2a668ee24c2882449))

## [1.3.13] - 2026-06-02

### Features

- **ffi:** structured tracing, error codes, and Rust→Qt log bridge ([17fb0d6](https://github.com/biplav00/snapforge/commit/17fb0d6e6fc7646a535f61c8efe2323127fb02ab))
- **hotkeys:** dynamic global shortcuts from shared config ([3d87c62](https://github.com/biplav00/snapforge/commit/3d87c6256ab75d8239116d1ed5ae9e519d426976))

### Tests

- **qt:** add QTest suite; scaffold universal-binary build ([30e05c5](https://github.com/biplav00/snapforge/commit/30e05c5e8aaabde1da5c4d8aa5f26a86e51153a3))

### Maintenance

- install ffmpeg for the app bundling step ([a57c14f](https://github.com/biplav00/snapforge/commit/a57c14fca2de01e5ce92ee55cbc565a67036d051))
- install dylibbundler so the Qt app build succeeds ([53671a9](https://github.com/biplav00/snapforge/commit/53671a90567f34465e5a9a932830f8790b336acb))
- enforce rustfmt, clippy, and cargo-deny gates ([28013a2](https://github.com/biplav00/snapforge/commit/28013a222cb098fbeb9091c95a29f425e9bca1f3))
- **release:** use build_dmg.sh as single dmg source of truth ([540571e](https://github.com/biplav00/snapforge/commit/540571e7729be3944905b2f6eb1e414ccb0200c8))

## [1.3.12] - 2026-05-31

### Features

- **logging:** comprehensive dev logger + log tab rebuild ([7e88217](https://github.com/biplav00/snapforge/commit/7e88217c865452cdb409f18f9ccc4c6e33094027))

### Fixes

- **overlay:** recover hotkey gate after failed/stalled capture ([8a50c54](https://github.com/biplav00/snapforge/commit/8a50c5488c9c43bb9ae1f0bc2e9cdfc6cb5c2725))

### Tests

- **capture/encode:** edge-case tests + fix FFI panic UB and CI compile error ([ab6e757](https://github.com/biplav00/snapforge/commit/ab6e757fd9610fd986080880c81eaf9e115ea14d))

### Maintenance

- **claude:** add project agents + release-helper skill ([c0a718e](https://github.com/biplav00/snapforge/commit/c0a718e4ecf4735e093c1b55f657c66574d74416))

## [1.3.11] - 2026-05-19

### Documentation

- **architecture:** audit + update for post-Phase-2 state ([562d1b0](https://github.com/biplav00/snapforge/commit/562d1b05b8555247258eb9413135d3cfa02e5040))

## [1.3.10] - 2026-05-19

### Features

- **app:** add save_prerendered use-case for annotated + clipboard-only paths ([023aef1](https://github.com/biplav00/snapforge/commit/023aef11be8fd84d00166f59f56cb55578066085))
- **ffi:** expose use-case surface (screenshot, recording, clicks) alongside primitives ([752b8f4](https://github.com/biplav00/snapforge/commit/752b8f42c4a44737160716edfcc790fb4d55242a))
- **crates:** add snapforge-app use-case layer ([f4d29fb](https://github.com/biplav00/snapforge/commit/f4d29fb5c69b1dad5c0f867dec5125dd58694744))

### Refactoring

- **ffi:** delete deprecated primitives now that all callers use the use-case surface ([1394063](https://github.com/biplav00/snapforge/commit/13940638b05e075b44a847cf72b835044b4b3624))
- **qt:** migrate annotated save + Cmd+C clipboard paths to snapforge_save_prerendered ([9513902](https://github.com/biplav00/snapforge/commit/9513902941902c837548be4e626bf89929b4593c))
- **qt:** replace ClickEventTap with ClickTap (uses snapforge_clicks_* FFI) ([060d727](https://github.com/biplav00/snapforge/commit/060d72704d3e8c505b5236c0fd683e417f9c2868))
- **qt:** migrate recording callsites to snapforge_record_* use-case FFI ([2be8a28](https://github.com/biplav00/snapforge/commit/2be8a28e113080407cb347e37f1d4067856dbe2e))
- **crates:** split snapforge-core into domain/capture/encode/storage; keep core as facade ([d7a0bdc](https://github.com/biplav00/snapforge/commit/d7a0bdc2a789fa71e1307c7da4f40632115f1978))

### Maintenance

- bump snapforge-app to 1.3.9 to match workspace ([f2fe2ec](https://github.com/biplav00/snapforge/commit/f2fe2ecea35896a4ee092c0fcf04bc19d79d6160))

## [1.3.9] - 2026-05-19

### Refactoring

- **recording:** extract recording lifecycle wiring into RecordingController ([16301e9](https://github.com/biplav00/snapforge/commit/16301e9c09dbf188cabafd05cd1a7dfcbe8fc391))
- **tray:** extract tray icon rendering + menu builders into TrayIcon class ([742f5c0](https://github.com/biplav00/snapforge/commit/742f5c04f9503d8f4cb4c13ecefc513a783f83a9))
- **layout:** carve qt/src into grouped folders; move scripts to packaging ([4c3c94d](https://github.com/biplav00/snapforge/commit/4c3c94d945a6e1432c5e79fe6a3252956ac50398))

## [1.3.8] - 2026-05-19

### Documentation

- **architecture:** seed mental-model docs for solo + AI workflow ([c15ec72](https://github.com/biplav00/snapforge/commit/c15ec72a1ef6575be962be7599b69e4ff2953e35))

## [1.3.7] - 2026-05-19

### Features

- **recording:** minimal tray pill + click visualizer ([569126e](https://github.com/biplav00/snapforge/commit/569126ec9db1fb9bd3f5c18ac9185fb46b66d7ca))

## [1.3.6] - 2026-05-10

### Fixes

- **tray:** shrink idle tray icon to 18pt with proportional glyph ([290f305](https://github.com/biplav00/snapforge/commit/290f305dd41f2462d303151806e023fa3da2e92f))

## [1.3.5] - 2026-05-10

### Fixes

- **capture:** try CGDisplayCreateImage first to capture fullscreen apps ([6982101](https://github.com/biplav00/snapforge/commit/69821017353932e65a0dc156d385d3ab162ca7af))
- correctness bugs found in audit ([76693a9](https://github.com/biplav00/snapforge/commit/76693a9fea3e35bfdf89afc2775592e55f69b2b5))
- **overlay:** run screen capture off the UI thread ([1aa9088](https://github.com/biplav00/snapforge/commit/1aa9088988db896f398a8982fe8aa1b44e93b76a))
- **ffi:** magic-tag recording handle, registry buffer free, atomic m_handle ([0666bee](https://github.com/biplav00/snapforge/commit/0666bee928cf98af6b7203df0d6588ea4fba40f6))
- **security:** pin actions, cap config size, harden ffmpeg argv ([6daca72](https://github.com/biplav00/snapforge/commit/6daca7217f5f087c86075da9c96c427b3597ca8e))

### Performance

- **record:** kill per-frame allocs and per-pixel f64 in click overlay ([bcdbf97](https://github.com/biplav00/snapforge/commit/bcdbf97890260f5348cb59a5cbefe07f83c08573))

### Tests

- ABI smoke suite and explicit headless skip gating ([ccb8226](https://github.com/biplav00/snapforge/commit/ccb82264d93ba8524520fd11a027d76daa9d1b77))

### Maintenance

- remove dead members and unused decls ([a9351a3](https://github.com/biplav00/snapforge/commit/a9351a3efb1d6883a537fc5d774bdbbca7487e8f))

## [1.3.4] - 2026-04-26

### Fixes

- **macos:** rename bundle to Snapforge.app, stabilize TCC, latch reprompts ([#33](https://github.com/biplav00/snapforge/pull/33))

## [1.3.3] - 2026-04-26

### Maintenance

- inline dmg build into release workflow ([#32](https://github.com/biplav00/snapforge/pull/32))

## [1.3.2] - 2026-04-26

### Features

- **qt:** region resize via edge/corner handles; fix click-outside reopen ([808f6ed](https://github.com/biplav00/snapforge/commit/808f6ed2ee50d64f3090f6a4d1e09fde6480398b))
- **qt:** fullscreen capture, tool toggle, Space follow, lock overlay ([ce6efea](https://github.com/biplav00/snapforge/commit/ce6efea0752a07826ca55e92bfd8b503733fb3ef))
- **qt:** add bundled app icons and DMG build script ([3a3c4fd](https://github.com/biplav00/snapforge/commit/3a3c4fd89cdf6470cf79ed02bb204d5d9cff6b2b))
- **qt:** macOS app bundle with DMG build target ([7dc32dc](https://github.com/biplav00/snapforge/commit/7dc32dcf3f06f905153ad0689dff3dda44fb2040))
- **qt:** recording mode, modern preferences, full hotkeys editor ([44366c1](https://github.com/biplav00/snapforge/commit/44366c1e23803b6af03c6e073198505f2c2a4003))
- **qt:** wire up tray menu with recording, history, preferences, hotkeys ([5fe2c49](https://github.com/biplav00/snapforge/commit/5fe2c4958a6f55d454ee8b29bb0edf52b3ecf89f))
- **qt:** add RecordingManager, HistoryWindow, and PreferencesWindow ([0fcbd87](https://github.com/biplav00/snapforge/commit/0fcbd87a7a12114d3ca64a0a6ead531ddf56b748))
- add recording, history, and config FFI functions ([4f373a0](https://github.com/biplav00/snapforge/commit/4f373a00abf5181344a20b7c455e75ee20782b1d))
- **qt:** integrate annotation system into overlay with composited save/copy ([d5d629d](https://github.com/biplav00/snapforge/commit/d5d629d30df3f69955f653b1b1f3a2d4acf1945a))
- **qt:** add AnnotationCanvas and AnnotationToolbar widgets ([6f3b4e8](https://github.com/biplav00/snapforge/commit/6f3b4e8c741efaa33400bef6b1d0f9041eb69d3e))
- **qt:** add annotation data types, state management, and renderer ([926dfb9](https://github.com/biplav00/snapforge/commit/926dfb99dde01ef98d781c8a43a1a9bdfa6db71a))
- add one-command build script for Qt frontend ([dd538e3](https://github.com/biplav00/snapforge/commit/dd538e3a173c63e84f3155313ac2e901af242c31))
- add Qt6 native frontend with overlay, global hotkey, and save ([a19200c](https://github.com/biplav00/snapforge/commit/a19200c66246bc1b8cbc6ed232dd566d560c51ee))
- add snapforge-ffi crate with C FFI wrappers over core ([0b692ca](https://github.com/biplav00/snapforge/commit/0b692ca16d9dee44a902f2d869635107b39520a6))
- ripple click animation in recordings ([bfcbbef](https://github.com/biplav00/snapforge/commit/bfcbbefae58da8264c4bd6dd9346dee219eb2b3a))
- redesign history window with search, filters, and video support ([ef1f462](https://github.com/biplav00/snapforge/commit/ef1f462e82e87d78183bce3b23fb8b2b0bfbb20d))
- dim outside recording region with dashed white border ([0503be6](https://github.com/biplav00/snapforge/commit/0503be66a643b46d95abe8bf6c81aba499efad15))
- visualize mouse clicks in screen recordings ([83284dd](https://github.com/biplav00/snapforge/commit/83284dd8544b175f2b08cc3fa16f0eead9be5d41))
- show red outline around region during recording ([1e6e0f5](https://github.com/biplav00/snapforge/commit/1e6e0f57612b1960994be10aef5ee0ef78de4826))
- add recordings to history and copy to clipboard on stop ([eb557d0](https://github.com/biplav00/snapforge/commit/eb557d0face5d3833c6433cbba7ce97871185a83))
- add proper color picker with hex input and native color chooser ([dedac88](https://github.com/biplav00/snapforge/commit/dedac88a83a88b1d65d7799589c6dc1ebb64991c))
- implement SCK screenshot capture via SCScreenshotManager ([ad5d4f0](https://github.com/biplav00/snapforge/commit/ad5d4f03bb22383ee5be5c01ccbcc244e964c633))
- rewrite macos capture to SCK — display enum and permission ([c5e92fe](https://github.com/biplav00/snapforge/commit/c5e92fe9a4eac5c11c06d5076a8043209106513b))

### Fixes

- **ci:** use repo-root path for release-please CMake extra-file ([#28](https://github.com/biplav00/snapforge/pull/28))
- **qt:** remember-last-region failed when saved region touched origin ([1570896](https://github.com/biplav00/snapforge/commit/157089695d0501b58b56ec7f8e1342432b3811ac))
- edge-case hardening + overlay screenshot freshness ([6110a86](https://github.com/biplav00/snapforge/commit/6110a862a563513ece83a6f528aa4d278faf675f))
- address code review findings across core, FFI, and Qt ([df15fe7](https://github.com/biplav00/snapforge/commit/df15fe7cfc68b2957514d2853fe894b65e76ac4e))
- **qt:** reliable overlay show/hide with fullscreen + desktop support ([31e757f](https://github.com/biplav00/snapforge/commit/31e757fe4e4d00240cf3a16678a1d07c746eb580))
- **qt:** overlay appears over fullscreen apps without Space switch ([8587ef5](https://github.com/biplav00/snapforge/commit/8587ef5e95747cacdda512170844f8f6677a2058))
- bundle ffmpeg as Tauri externalBin sidecar ([ca74760](https://github.com/biplav00/snapforge/commit/ca7476003cde4468b9f6ca51862619b21b34f8a0))
- remove unsafe setCollectionBehavior FFI causing app crash ([29f30ea](https://github.com/biplav00/snapforge/commit/29f30ea66b95850c8db0fa9bf985570825a66eb1))
- dispatch tray and window operations to main thread ([7bb4913](https://github.com/biplav00/snapforge/commit/7bb4913cb10d100df59ccd0cea32a98fa2d9a3b7))
- multiple recording fixes ([cf6a182](https://github.com/biplav00/snapforge/commit/cf6a1821abcfcffc424b861bead528710d9c3299))
- position toolbar inside region when no space outside ([3a30293](https://github.com/biplav00/snapforge/commit/3a30293653072fca6107ac4108ee67a97ee215ee))
- query display scale factor dynamically instead of hardcoding 2.0 ([169bdc4](https://github.com/biplav00/snapforge/commit/169bdc4255e8684669acf2632fde0eeae395295c))
- add core:window:allow-start-dragging permission ([f0b001c](https://github.com/biplav00/snapforge/commit/f0b001c13d413edb6bb8d439571722a279b0f3e0))
- record region button click + draggable recording indicator ([bad8496](https://github.com/biplav00/snapforge/commit/bad84963516a89f2a6fe5f523ffecddffc5447f5))
- crop recording frames to even dimensions for libx264 yuv420p ([3d48da4](https://github.com/biplav00/snapforge/commit/3d48da4a4e3a420e2d0075483ff9af5b827f7c06))
- run start_recording on background thread to avoid SCK deadlock ([f170561](https://github.com/biplav00/snapforge/commit/f170561eaf9baf06c10322412db0c2619eaa68e9))
- add CaptureContext for fast repeated captures during recording ([ba34a1a](https://github.com/biplav00/snapforge/commit/ba34a1a82a3b6cad2966168882ee5641bb0f9788))
- use CGDisplayMode pixel dimensions for correct Retina resolution ([afc29dc](https://github.com/biplav00/snapforge/commit/afc29dc3b51035a328ad1294a6fc5e4c2fe0ba5b))
- capture primary display only, remove debug logging ([5aa75e4](https://github.com/biplav00/snapforge/commit/5aa75e47d4868754bb8c5244ef608e6704e5cff4))
- skip preflight permission check — try capture directly instead ([03ae537](https://github.com/biplav00/snapforge/commit/03ae537b058fbf07f175f7435ce11c2a0b0c34e3))
- move trigger_screenshot/recording off main thread to prevent SCK deadlock ([d309c32](https://github.com/biplav00/snapforge/commit/d309c32e561b0b6aebdbd14622e6c181d1ab0419))
- run SCK capture on background thread to avoid main thread deadlock ([07d5a42](https://github.com/biplav00/snapforge/commit/07d5a42a1e5a0448ae1629bf73bc5149d84539f1))
- address code review issues for SCK capture ([306503c](https://github.com/biplav00/snapforge/commit/306503c985fa7ba5816c820259fedccb7e3ec934))
- permission check fallback — try real capture if preflight fails ([71e801a](https://github.com/biplav00/snapforge/commit/71e801a65bb4e9665068b6969b632962553f1dfa))

### Performance

- async capture and pre-warm overlay for 2ms hotkey response ([9fe64b4](https://github.com/biplav00/snapforge/commit/9fe64b44b0a2a7aae287aac5395fc7704d0809e4))
- fast BGRA recording path with downscaling for macOS ([2facd56](https://github.com/biplav00/snapforge/commit/2facd56ffb90ccf27421f668ad782bd91693c619))
- remove prewarm overlay hack — SCK capture is fast enough ([0495cc9](https://github.com/biplav00/snapforge/commit/0495cc937c6c41e4c1a298a2ce7daa520b8d9a13))

### Refactoring

- merge step and callout into single steps tool ([45a9b27](https://github.com/biplav00/snapforge/commit/45a9b27960582c0d4df90d7ac5939c7ebf7f9751))

### Documentation

- add recording, history, preferences design spec ([a91ce21](https://github.com/biplav00/snapforge/commit/a91ce21e0a27bb6ace928dbbf2b5ece0fd60ae3b))
- add ScreenCaptureKit migration implementation plan ([4d9a824](https://github.com/biplav00/snapforge/commit/4d9a824d503d64ccf262d3bfcce0a0f366adaea4))
- add ScreenCaptureKit migration design spec ([0663b3b](https://github.com/biplav00/snapforge/commit/0663b3b01b2f99f6b9d9aa7f50fb3ea6f7318b42))

### Tests

- update macOS capture tests for SCK migration ([5f560f8](https://github.com/biplav00/snapforge/commit/5f560f8a05a2a8ff0f87c2ea06d6b17bea3bb6bd))

### Maintenance

- **release:** match auto-commit by prefix not substring ([#31](https://github.com/biplav00/snapforge/pull/31))
- **release:** replace skip token to avoid GitHub-reserved sentinel ([#30](https://github.com/biplav00/snapforge/pull/30))
- docs cleanup + auto-release on every push to main ([#29](https://github.com/biplav00/snapforge/pull/29))
- add macOS CI, release-please, and auto-dmg release ([#27](https://github.com/biplav00/snapforge/pull/27))
- drop CLI crate and trim unused capture/permission helpers ([46d008e](https://github.com/biplav00/snapforge/commit/46d008e6bad626e25d0cb4258f1756f700922092))
- remove dead code and Tauri/npm leftovers ([8a88657](https://github.com/biplav00/snapforge/commit/8a886576406b5dc27726ebd60fed752090ede4ef))
- remove Tauri/Svelte legacy after Qt migration ([dfcd792](https://github.com/biplav00/snapforge/commit/dfcd7922f5a24ad84dfae884df12166f6e719256))
- remove dead CaptureWorker class, add design docs ([1720a78](https://github.com/biplav00/snapforge/commit/1720a78bf0f4398ce57b69ba141d9e3a369c8d22))

### Other Changes

- add objc2-screen-capture-kit for SCK capture migration ([4fe3931](https://github.com/biplav00/snapforge/commit/4fe393175fcce1b1e3adfc582b7552750548d681))

## [1.3.1] - 2026-04-08

### Fixes

- check screen recording permission before capturing ([aaa7157](https://github.com/biplav00/snapforge/commit/aaa71574ee8fbb05107a2f2160eab7dc0316fb72))
- sync all version strings to 1.3.0, fix release-please version bumping ([a7c5cbc](https://github.com/biplav00/snapforge/commit/a7c5cbc03ed9f2587a5ee1590806790af1405bf8))
- capture screen BEFORE overlay opens — fixes wallpaper-only screenshot ([400ddeb](https://github.com/biplav00/snapforge/commit/400ddeb9cfb096b5e36eaeac271f8efd31574a70))
- suppress Node.js 20 deprecation warning in release-please ([22e1b6c](https://github.com/biplav00/snapforge/commit/22e1b6cb698e68ce7c48f1de7b61fa5172aba5bf))

### Maintenance

- **main:** release 1.3.1 ([7ff2997](https://github.com/biplav00/snapforge/commit/7ff2997b1a02fff6a5c28268118584da29a35b65))

## [1.3.0] - 2026-04-08

### Features

- add Windows build (.msi + .exe) to release pipeline ([42c28ca](https://github.com/biplav00/snapforge/commit/42c28ca72a6bf8059c5acf34472ee605a8902422))

### Fixes

- remove Intel build, ARM64 only ([c37a455](https://github.com/biplav00/snapforge/commit/c37a455ffdc13519f285a207bec7ac7ca07a65a0))

### Performance

- pre-warm webview at startup for instant overlay ([f6b3bfc](https://github.com/biplav00/snapforge/commit/f6b3bfc572b328a82ba861d57a2a07e28f414cf4))
- instant overlay + fix tray quit ([7d2866c](https://github.com/biplav00/snapforge/commit/7d2866cdabbc2a8cf636a4b8b8a50d44e12cceb4))
- app runtime optimizations for production ([e9a3355](https://github.com/biplav00/snapforge/commit/e9a33550a79305bc93361f3d50c5feae88753bfb))
- speed up CI builds — incremental compilation, thin LTO, shared cache keys ([0987e31](https://github.com/biplav00/snapforge/commit/0987e318a8cd2dc1199edaa76adc1a7da1632925))

### Maintenance

- **main:** release 1.3.0 ([3f3c2a1](https://github.com/biplav00/snapforge/commit/3f3c2a1635f97b3a5e45ced24843d445e7794a3b))

## [1.2.4] - 2026-04-08

### Fixes

- skip code signing in CI (no certificate configured yet) ([6f1097c](https://github.com/biplav00/snapforge/commit/6f1097c1ec96449d7afb8498cca73f13c7f5a682))

### Maintenance

- **main:** release 1.2.4 ([2859e37](https://github.com/biplav00/snapforge/commit/2859e37683a73393b87a35f6768f0b443eee5084))

## [1.2.3] - 2026-04-08

### Fixes

- overhaul CI pipeline — direct dmg build, no tauri-action ([4b7956e](https://github.com/biplav00/snapforge/commit/4b7956e18aac30ad45d8c3d1352a575c82241f59))

### Maintenance

- **main:** release 1.2.3 ([52b0b09](https://github.com/biplav00/snapforge/commit/52b0b09a9ee1fdfaa7cf8f3b59fae4aee1436524))

## [1.2.2] - 2026-04-08

### Fixes

- resolve all Svelte build warnings — zero warnings clean build ([9aacacc](https://github.com/biplav00/snapforge/commit/9aacaccbd2e54f281816da5884e7ca382852ce67))
- add @tauri-apps/cli as dev dependency for CI builds ([61e347e](https://github.com/biplav00/snapforge/commit/61e347e8a835a4579af3e12d7e490f7385e73289))
- streamline all workflows ([f9ef738](https://github.com/biplav00/snapforge/commit/f9ef738ba741c560beab458051a84fce3713b186))

### Maintenance

- **deps:** bump the all-cargo group with 2 updates ([5761019](https://github.com/biplav00/snapforge/commit/5761019cf8236ced36054d72f32b0e1e38d7b218))
- **deps-dev:** bump the all-npm group with 2 updates ([3edc78e](https://github.com/biplav00/snapforge/commit/3edc78ed43bbf307124e0c2eb26d1479c057d7f6))
- **main:** release 1.2.2 ([f8224d9](https://github.com/biplav00/snapforge/commit/f8224d95bae82546e77c7e33f54b46343dcf0e8e))

## [1.2.1] - 2026-04-08

### Fixes

- use tauri-action@dev for Tauri v2, macOS only builds for now ([c5a09a1](https://github.com/biplav00/snapforge/commit/c5a09a1a0d64e13ed6f4a854f5ad90ad61bcf313))

### Maintenance

- **main:** release 1.2.1 ([58131c9](https://github.com/biplav00/snapforge/commit/58131c9593352c5f34c38b4be29b32dc3d94a6ed))

## [1.2.0] - 2026-04-08

### Fixes

- CI build — fix Windows clipboard, remove source archives from releases ([aa5ae37](https://github.com/biplav00/snapforge/commit/aa5ae3785e795ac811f1fa67882cb794d6bbb10d))
- streamline CI pipeline — parallel jobs, build checks, no duplication ([163a470](https://github.com/biplav00/snapforge/commit/163a47041716c76058a356e396feb6e0ce825890))

### Maintenance

- **main:** release 1.2.0 ([ca861b5](https://github.com/biplav00/snapforge/commit/ca861b5887251e2b1d580eca031ec702f5c91ce1))

## [1.1.0] - 2026-04-08

### Features

- performance optimization — native clipboard, parallel capture, RAF canvas, async history ([b185713](https://github.com/biplav00/snapforge/commit/b1857137bb714c819c27dd1e62513ac136ac9850))

### Fixes

- build apps in release-please workflow instead of tag-triggered workflow ([86a496a](https://github.com/biplav00/snapforge/commit/86a496a2b98f0f06476c33801d255e58160e7617))

### Maintenance

- **main:** release 1.1.0 ([6761ebc](https://github.com/biplav00/snapforge/commit/6761ebcfbe3a14733c4d856699f7d0d36eafcf06))

## [1.0.0] - 2026-04-08

### Features

- major enhancements — cross-platform, multi-monitor, history, annotations, tooling ([e366a09](https://github.com/biplav00/snapforge/commit/e366a091f8bf57f14dab481bc2bda7541a6bdf36))
- add CI/CD, Biome linting, semantic release, and Dependabot ([8d406ce](https://github.com/biplav00/snapforge/commit/8d406ce565b59b44c2471cea74fb4027352543aa))
- rename project from ScreenSnap to Snapforge ([d83c526](https://github.com/biplav00/snapforge/commit/d83c5268e9dd858d196018d4755c5408ba3ff849))
- Flameshot-style annotation — draw directly on transparent overlay ([8653794](https://github.com/biplav00/snapforge/commit/86537944f05e7ba637ff2874bfdfbef7df6e1c57))
- custom UI controls — toggle switches, radio dots, styled slider ([8306122](https://github.com/biplav00/snapforge/commit/8306122ab789ffd13e427edd3f099ca504423127))
- support bundled FFmpeg — find bundled binary, fall back to system PATH ([81fb78b](https://github.com/biplav00/snapforge/commit/81fb78b3b349a59cba5a8f130217810d42fc2e2b))
- all hotkeys configurable, system theme support in preferences ([a93090b](https://github.com/biplav00/snapforge/commit/a93090b8297c2a502713ddc38b445569cd325363))
- polish preferences UI — add Recording tab, fix hotkeys, improve styling ([786844b](https://github.com/biplav00/snapforge/commit/786844bc575831dbd228b1edfd1b5e5d3a1ee129))
- unique app icon — diagonal snap slash with corner brackets ([ca77f11](https://github.com/biplav00/snapforge/commit/ca77f1149650ac99c0e6a9f3be073e6d2ff1823f))
- add app icon — viewfinder with crosshair and snap bolt ([8c3f3d4](https://github.com/biplav00/snapforge/commit/8c3f3d4f44667ba4ed6dcc3ca1bd6a60e90b6fa1))
- Lightshot-style transparent overlay — no pre-captured screenshot ([e604fc5](https://github.com/biplav00/snapforge/commit/e604fc539ded72abb6052935d6c2876b79cd2fde))
- add region selection for recording via overlay ([5de65b3](https://github.com/biplav00/snapforge/commit/5de65b329f42aae1a136a682b056795bd978af23))
- add CLI recording — screen record --fullscreen/--region with Ctrl+C stop ([975ea63](https://github.com/biplav00/snapforge/commit/975ea630a73d751d34252b37e7f20fdc84bf30e8))
- add recording indicator UI — red dot, timer, stop button ([a60b34d](https://github.com/biplav00/snapforge/commit/a60b34dd31cb8f8f49094859b59a83a85229fa76))
- add Record Screen to tray menu and global hotkey ([06ca102](https://github.com/biplav00/snapforge/commit/06ca1027f796d4dd465099db680b4afaf7bac882))
- add Tauri recording state and start/stop commands ([5cc2274](https://github.com/biplav00/snapforge/commit/5cc22745966b2e13eb2930aebe8f12d03bd150cf))
- add recording module — FFmpeg frame piping with start/stop API ([26b8659](https://github.com/biplav00/snapforge/commit/26b865963926b9d5139078fa3f5ce84887e3ff3c))
- add recording config — format, fps, quality, hotkey ([6f7dc47](https://github.com/biplav00/snapforge/commit/6f7dc471b1c77e47f8179d0a947fe1f432f07cc1))
- register all new tools — update registry, toolbar, canvas, shortcuts ([61f5a9f](https://github.com/biplav00/snapforge/commit/61f5a9fb2bc215617ba469cccc8c4bb93d586f71))
- add color picker (eyedropper) tool ([17706f1](https://github.com/biplav00/snapforge/commit/17706f1b1eff228540f89e146b5bfd1fccd62980))
- add blur/pixelate and measurement annotation tools ([9026f03](https://github.com/biplav00/snapforge/commit/9026f03601310a07d7f15baf31fa8520618a462d))
- add circle, highlight, and step number annotation tools ([8bd1a9a](https://github.com/biplav00/snapforge/commit/8bd1a9a4f1afa24ed336c0c332047029331bfda1))
- extend annotation types with circle, highlight, steps, blur, measure, colorpicker ([228aa4f](https://github.com/biplav00/snapforge/commit/228aa4f29adc58ac9ed95b53bdc777de68b0d86c))
- add preferences stylesheet with opaque white background ([68a4ff0](https://github.com/biplav00/snapforge/commit/68a4ff08b0db7915def524b8e7bbe887ff0720f3))
- add preferences UI — General, Hotkeys, Screenshots tabs ([e9780d2](https://github.com/biplav00/snapforge/commit/e9780d2e7607dfe8421af59cb646643f24717a6a))
- scaffold preferences window with Vite multi-page build ([02de611](https://github.com/biplav00/snapforge/commit/02de61146cf2eee46b3a4cfb523e0d18f5b5a36c))
- add global hotkeys — screenshot and last-region capture ([2c7c18b](https://github.com/biplav00/snapforge/commit/2c7c18bbb616f4210a9a1490919f583d4fba8dd2))
- add system tray with menu and on-demand overlay window ([4b6d50f](https://github.com/biplav00/snapforge/commit/4b6d50ff5862ef36853f43042bc4755b74f0424f))
- add Tauri commands for config read/write and open save folder ([8ce35d1](https://github.com/biplav00/snapforge/commit/8ce35d178e6fdb41f7b09ab14c3e492251dcae4b))
- add hotkey bindings to AppConfig ([a6a6ea2](https://github.com/biplav00/snapforge/commit/a6a6ea2c9e4029dc484ca682b75612f997615d45))
- auto-annotate after selection, tool shortcuts, fix text tool, copy shortcut ([49a4bde](https://github.com/biplav00/snapforge/commit/49a4bdee6a44743c9b27d118ff33dc7bc824076d))
- integrate annotation layer into overlay with annotate mode ([f037bb1](https://github.com/biplav00/snapforge/commit/f037bb1f0394994f4025a4d69a676f973745102f))
- add compositing module and Tauri commands for annotated save/copy ([f2b7589](https://github.com/biplav00/snapforge/commit/f2b7589882a80b242a37eeecb33f1eaf98aa675a))
- add floating annotation toolbar — tools, color, size, undo/redo, actions ([9f0e224](https://github.com/biplav00/snapforge/commit/9f0e2246a3ea4b2971eb0cbb2a07b7411906e70a))
- add annotation canvas — renders annotations and delegates to tools ([e682916](https://github.com/biplav00/snapforge/commit/e6829168d3f6a82ef5317d8434977198c30f39c3))
- add text annotation tool ([ec73cbf](https://github.com/biplav00/snapforge/commit/ec73cbf1d6a45dcb1e87d429daf6cda16fd21623))
- add annotation tools — arrow, rect, line, freehand with registry ([4fb8c82](https://github.com/biplav00/snapforge/commit/4fb8c82641908c399e30477438198adf7d3bde49))
- add annotation state management with undo/redo ([aa33c90](https://github.com/biplav00/snapforge/commit/aa33c902f7fe4a905332c17c4582d3bce76719c1))
- add annotation types and tool interface ([8417a0e](https://github.com/biplav00/snapforge/commit/8417a0e9844f4bbe02bb0a64666723f248bc899b))
- CLI launches Tauri overlay for interactive region selection ([10d60ab](https://github.com/biplav00/snapforge/commit/10d60aba5c0702500c656b9f5f3a61b3efaccdc5))
- add region selector — draw, resize, move with save/cancel ([7fda9c6](https://github.com/biplav00/snapforge/commit/7fda9c64a2d3331bf2ecd6222ee9cf5e8e77779d))
- add overlay component — captures and displays frozen screenshot ([54b54a2](https://github.com/biplav00/snapforge/commit/54b54a234166f532eeb381d96aafa6f343168fd0))
- scaffold Tauri v2 app with overlay window and capture commands ([10a802b](https://github.com/biplav00/snapforge/commit/10a802b89fbdc941c2dc60ff582e46d65b1fd43a))
- scaffold Svelte + Vite frontend for Tauri ([6988faa](https://github.com/biplav00/snapforge/commit/6988faa34c8d464fd106304cdde89e6c3ae09af4))
- complete Phase 1 — format, clipboard, public API, CLI, and integration tests ([ba97799](https://github.com/biplav00/snapforge/commit/ba97799bb98abc139f3c35e540a4919e9b39e970))
- add screen capture — trait + macOS CoreGraphics implementation ([f7fd3a2](https://github.com/biplav00/snapforge/commit/f7fd3a277933943ce88f4f5fef89f6241b947a01))
- add config module — load, save, filename generation ([9e8e4dc](https://github.com/biplav00/snapforge/commit/9e8e4dc82f3a73cd9db5aa772736b30b28ab5ace))
- add core types — Rect, CaptureFormat, LastRegion ([bfdee24](https://github.com/biplav00/snapforge/commit/bfdee24e45199f405c2ca4b2a30b162be3c2cdb3))
- scaffold Cargo workspace with screen-core and cli crates ([b5cb38a](https://github.com/biplav00/snapforge/commit/b5cb38a20e3756524222638b4458947014632b11))

### Fixes

- release-please config — use typed updaters for Cargo.toml and tauri.conf.json ([023e1ed](https://github.com/biplav00/snapforge/commit/023e1ed306cdef3b6285cc5f43595539701ad5d4))
- CI test — display_count may be 0 in headless CI environments ([3f594a8](https://github.com/biplav00/snapforge/commit/3f594a82a89f798259aad2fa3cbb48af380cc1dc))
- CI clippy on Linux — remove unused Manager import and needless return ([f1fa155](https://github.com/biplav00/snapforge/commit/f1fa1552e0ff895e6459fafe0c8b61ef5f9fee56))
- CI typecheck — enable allowImportingTsExtensions, fix renderAnnotation type ([347ef36](https://github.com/biplav00/snapforge/commit/347ef36fae296f5e2ae9b2cf8bdbf214fc1dbfd8))
- CI failures + add husky pre-commit, README, lint fixes ([c6a5602](https://github.com/biplav00/snapforge/commit/c6a5602b264a1e1db17cac194719b616e9b96581))
- audit fixes — critical, important, and minor issues ([d27d946](https://github.com/biplav00/snapforge/commit/d27d9460b6d46f879596131355e86b5da5c6c190))
- edge-to-edge overlay without macOS fullscreen Space ([eb30c26](https://github.com/biplav00/snapforge/commit/eb30c2660bb83b555f1cf18be1bc74458d7e897e))
- remove FFmpeg mention from Recording preferences tab ([827cbdc](https://github.com/biplav00/snapforge/commit/827cbdc7661252effae5f25afd77388819d613d0))
- remove ScreenSnap Preferences header text ([854d7ae](https://github.com/biplav00/snapforge/commit/854d7ae605ac811545f18227b04d71c1f3a938a8))
- add serde(default) to config structs for backwards compatibility ([2d1fd28](https://github.com/biplav00/snapforge/commit/2d1fd2803e4abb2b630829739387ca5d6803904b))
- remove tab icons, show all hotkeys in preferences ([32b5d5c](https://github.com/biplav00/snapforge/commit/32b5d5c0b47e2462e2010794fa483782ce4eb570))
- tray icon — transparent template icon for light/dark macOS themes ([02da838](https://github.com/biplav00/snapforge/commit/02da8384f04db52d923c0d7f67669ce574faf7bb))
- show single colored tray icon instead of two white boxes ([af0d972](https://github.com/biplav00/snapforge/commit/af0d9726a9a25e24fa19e97f4b9ecb47434128dd))
- show app icon in system tray ([d604836](https://github.com/biplav00/snapforge/commit/d604836306656ace680a172cc65ac3f8b7139e9f))
- show copy toast inside region, stay in annotation mode after copy ([a776ebd](https://github.com/biplav00/snapforge/commit/a776ebd4c46f6053ec12372b733b9433f9057305))
- capture screen before overlay window opens ([12f868a](https://github.com/biplav00/snapforge/commit/12f868af0abfbf4c53299411587acfb0138f6641))

### Documentation

- add original design spec and Phase 1 plan ([179cb68](https://github.com/biplav00/snapforge/commit/179cb6835d35a88936cd82064911e0a0752ab7bf))
- add Phase 2d implementation plan — screen recording with FFmpeg ([0470364](https://github.com/biplav00/snapforge/commit/0470364090b687263a0af80be299b6870530aa16))
- add Phase 2b+ plan — 6 additional annotation tools ([a741ea5](https://github.com/biplav00/snapforge/commit/a741ea50a6c5639193e48def1d42b527eb576fd3))
- add Phase 2c implementation plan — tray, hotkeys, preferences ([98f3008](https://github.com/biplav00/snapforge/commit/98f3008f2fc1daf1d82397d041c5cf53e9f6019b))
- add Phase 2b implementation plan — annotation toolkit ([474742b](https://github.com/biplav00/snapforge/commit/474742b4d334ee979cf9ab6637da19223f3db215))
- add Phase 2b design spec — annotation toolkit (core subset) ([a448fe2](https://github.com/biplav00/snapforge/commit/a448fe2d7c3c4fad8dd95dc6b7f51985ec186034))
- add Phase 2a implementation plan — Tauri overlay + region selection ([c719c72](https://github.com/biplav00/snapforge/commit/c719c72abe931712a87f58cfc7e9e1de02a1882c))

### Maintenance

- **main:** release 1.0.0 ([6eff1f1](https://github.com/biplav00/snapforge/commit/6eff1f1069d93e5d29ba6bafc318231c996dd218))
- trigger release-please ([241449d](https://github.com/biplav00/snapforge/commit/241449de29e8503a4757a20a61eeab11ef8f1786))
- **deps:** bump core-graphics from 0.24.0 to 0.25.0 ([63c4ceb](https://github.com/biplav00/snapforge/commit/63c4cebabfc1167ef96f5b70060f26bbbb6a0376))
- **deps:** bump actions/checkout from 4 to 6 ([ab23497](https://github.com/biplav00/snapforge/commit/ab23497a089aaca68790f0d18a4b122d9f480ece))
- **deps:** bump actions/setup-node from 4 to 6 ([349e030](https://github.com/biplav00/snapforge/commit/349e0303c69505997bf01480e6bd2077b4eff6fe))
- update .gitignore — ignore Cargo.lock, dist, node_modules, gen ([b6775e7](https://github.com/biplav00/snapforge/commit/b6775e723d819c42f4a9ab8e77ca66f47cf1eae0))

