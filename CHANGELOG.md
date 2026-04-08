# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.3](https://github.com/biplav00/snapforge/compare/v1.2.2...v1.2.3) (2026-04-08)


### Bug Fixes

* overhaul CI pipeline — direct dmg build, no tauri-action ([4b7956e](https://github.com/biplav00/snapforge/commit/4b7956e18aac30ad45d8c3d1352a575c82241f59))

## [1.2.2](https://github.com/biplav00/snapforge/compare/v1.2.1...v1.2.2) (2026-04-08)


### Bug Fixes

* add @tauri-apps/cli as dev dependency for CI builds ([61e347e](https://github.com/biplav00/snapforge/commit/61e347e8a835a4579af3e12d7e490f7385e73289))
* CI build — add @tauri-apps/cli for npx tauri ([2fdcf10](https://github.com/biplav00/snapforge/commit/2fdcf106cec922133ce87a839bf44af03f55a5ec))

## [1.2.1](https://github.com/biplav00/snapforge/compare/v1.2.0...v1.2.1) (2026-04-08)


### Bug Fixes

* tauri-action v2 compatibility + macOS-only builds ([a35b053](https://github.com/biplav00/snapforge/commit/a35b053e0c09c4987043883be64748acd5ab105f))
* use tauri-action@dev for Tauri v2, macOS only builds for now ([c5a09a1](https://github.com/biplav00/snapforge/commit/c5a09a1a0d64e13ed6f4a854f5ad90ad61bcf313))

## [1.2.0](https://github.com/biplav00/snapforge/compare/v1.1.0...v1.2.0) (2026-04-08)


### Features

* performance optimization + streamlined CI ([80876c9](https://github.com/biplav00/snapforge/commit/80876c9338cae7f24d11e74e0c150c5cbdc0d152))


### Bug Fixes

* CI build — fix Windows clipboard, remove source archives from releases ([aa5ae37](https://github.com/biplav00/snapforge/commit/aa5ae3785e795ac811f1fa67882cb794d6bbb10d))
* streamline CI pipeline — parallel jobs, build checks, no duplication ([163a470](https://github.com/biplav00/snapforge/commit/163a47041716c76058a356e396feb6e0ce825890))

## [1.1.0](https://github.com/biplav00/snapforge/compare/v1.0.0...v1.1.0) (2026-04-08)


### Features

* performance optimization — native clipboard, parallel capture, RAF canvas ([b9ab11d](https://github.com/biplav00/snapforge/commit/b9ab11dc0222f87a0194259074c567f781a4b59a))
* performance optimization — native clipboard, parallel capture, RAF canvas, async history ([b185713](https://github.com/biplav00/snapforge/commit/b1857137bb714c819c27dd1e62513ac136ac9850))


### Bug Fixes

* build apps in release-please workflow instead of tag-triggered workflow ([86a496a](https://github.com/biplav00/snapforge/commit/86a496a2b98f0f06476c33801d255e58160e7617))

## 1.0.0 (2026-04-08)


### Features

* add annotation canvas — renders annotations and delegates to tools ([e682916](https://github.com/biplav00/snapforge/commit/e6829168d3f6a82ef5317d8434977198c30f39c3))
* add annotation state management with undo/redo ([aa33c90](https://github.com/biplav00/snapforge/commit/aa33c902f7fe4a905332c17c4582d3bce76719c1))
* add annotation tools — arrow, rect, line, freehand with registry ([4fb8c82](https://github.com/biplav00/snapforge/commit/4fb8c82641908c399e30477438198adf7d3bde49))
* add annotation types and tool interface ([8417a0e](https://github.com/biplav00/snapforge/commit/8417a0e9844f4bbe02bb0a64666723f248bc899b))
* add app icon — viewfinder with crosshair and snap bolt ([8c3f3d4](https://github.com/biplav00/snapforge/commit/8c3f3d4f44667ba4ed6dcc3ca1bd6a60e90b6fa1))
* add blur/pixelate and measurement annotation tools ([9026f03](https://github.com/biplav00/snapforge/commit/9026f03601310a07d7f15baf31fa8520618a462d))
* add CI/CD, Biome linting, semantic release, and Dependabot ([8d406ce](https://github.com/biplav00/snapforge/commit/8d406ce565b59b44c2471cea74fb4027352543aa))
* add circle, highlight, and step number annotation tools ([8bd1a9a](https://github.com/biplav00/snapforge/commit/8bd1a9a4f1afa24ed336c0c332047029331bfda1))
* add CLI recording — screen record --fullscreen/--region with Ctrl+C stop ([975ea63](https://github.com/biplav00/snapforge/commit/975ea630a73d751d34252b37e7f20fdc84bf30e8))
* add color picker (eyedropper) tool ([17706f1](https://github.com/biplav00/snapforge/commit/17706f1b1eff228540f89e146b5bfd1fccd62980))
* add compositing module and Tauri commands for annotated save/copy ([f2b7589](https://github.com/biplav00/snapforge/commit/f2b7589882a80b242a37eeecb33f1eaf98aa675a))
* add config module — load, save, filename generation ([9e8e4dc](https://github.com/biplav00/snapforge/commit/9e8e4dc82f3a73cd9db5aa772736b30b28ab5ace))
* add core types — Rect, CaptureFormat, LastRegion ([bfdee24](https://github.com/biplav00/snapforge/commit/bfdee24e45199f405c2ca4b2a30b162be3c2cdb3))
* add floating annotation toolbar — tools, color, size, undo/redo, actions ([9f0e224](https://github.com/biplav00/snapforge/commit/9f0e2246a3ea4b2971eb0cbb2a07b7411906e70a))
* add global hotkeys — screenshot and last-region capture ([2c7c18b](https://github.com/biplav00/snapforge/commit/2c7c18bbb616f4210a9a1490919f583d4fba8dd2))
* add hotkey bindings to AppConfig ([a6a6ea2](https://github.com/biplav00/snapforge/commit/a6a6ea2c9e4029dc484ca682b75612f997615d45))
* add overlay component — captures and displays frozen screenshot ([54b54a2](https://github.com/biplav00/snapforge/commit/54b54a234166f532eeb381d96aafa6f343168fd0))
* add preferences stylesheet with opaque white background ([68a4ff0](https://github.com/biplav00/snapforge/commit/68a4ff08b0db7915def524b8e7bbe887ff0720f3))
* add preferences UI — General, Hotkeys, Screenshots tabs ([e9780d2](https://github.com/biplav00/snapforge/commit/e9780d2e7607dfe8421af59cb646643f24717a6a))
* add Record Screen to tray menu and global hotkey ([06ca102](https://github.com/biplav00/snapforge/commit/06ca1027f796d4dd465099db680b4afaf7bac882))
* add recording config — format, fps, quality, hotkey ([6f7dc47](https://github.com/biplav00/snapforge/commit/6f7dc471b1c77e47f8179d0a947fe1f432f07cc1))
* add recording indicator UI — red dot, timer, stop button ([a60b34d](https://github.com/biplav00/snapforge/commit/a60b34dd31cb8f8f49094859b59a83a85229fa76))
* add recording module — FFmpeg frame piping with start/stop API ([26b8659](https://github.com/biplav00/snapforge/commit/26b865963926b9d5139078fa3f5ce84887e3ff3c))
* add region selection for recording via overlay ([5de65b3](https://github.com/biplav00/snapforge/commit/5de65b329f42aae1a136a682b056795bd978af23))
* add region selector — draw, resize, move with save/cancel ([7fda9c6](https://github.com/biplav00/snapforge/commit/7fda9c64a2d3331bf2ecd6222ee9cf5e8e77779d))
* add screen capture — trait + macOS CoreGraphics implementation ([f7fd3a2](https://github.com/biplav00/snapforge/commit/f7fd3a277933943ce88f4f5fef89f6241b947a01))
* add system tray with menu and on-demand overlay window ([4b6d50f](https://github.com/biplav00/snapforge/commit/4b6d50ff5862ef36853f43042bc4755b74f0424f))
* add Tauri commands for config read/write and open save folder ([8ce35d1](https://github.com/biplav00/snapforge/commit/8ce35d178e6fdb41f7b09ab14c3e492251dcae4b))
* add Tauri recording state and start/stop commands ([5cc2274](https://github.com/biplav00/snapforge/commit/5cc22745966b2e13eb2930aebe8f12d03bd150cf))
* add text annotation tool ([ec73cbf](https://github.com/biplav00/snapforge/commit/ec73cbf1d6a45dcb1e87d429daf6cda16fd21623))
* all hotkeys configurable, system theme support in preferences ([a93090b](https://github.com/biplav00/snapforge/commit/a93090b8297c2a502713ddc38b445569cd325363))
* auto-annotate after selection, tool shortcuts, fix text tool, copy shortcut ([49a4bde](https://github.com/biplav00/snapforge/commit/49a4bdee6a44743c9b27d118ff33dc7bc824076d))
* CLI launches Tauri overlay for interactive region selection ([10d60ab](https://github.com/biplav00/snapforge/commit/10d60aba5c0702500c656b9f5f3a61b3efaccdc5))
* complete Phase 1 — format, clipboard, public API, CLI, and integration tests ([ba97799](https://github.com/biplav00/snapforge/commit/ba97799bb98abc139f3c35e540a4919e9b39e970))
* custom UI controls — toggle switches, radio dots, styled slider ([8306122](https://github.com/biplav00/snapforge/commit/8306122ab789ffd13e427edd3f099ca504423127))
* extend annotation types with circle, highlight, steps, blur, measure, colorpicker ([228aa4f](https://github.com/biplav00/snapforge/commit/228aa4f29adc58ac9ed95b53bdc777de68b0d86c))
* Flameshot-style annotation — draw directly on transparent overlay ([8653794](https://github.com/biplav00/snapforge/commit/86537944f05e7ba637ff2874bfdfbef7df6e1c57))
* integrate annotation layer into overlay with annotate mode ([f037bb1](https://github.com/biplav00/snapforge/commit/f037bb1f0394994f4025a4d69a676f973745102f))
* Lightshot-style transparent overlay — no pre-captured screenshot ([e604fc5](https://github.com/biplav00/snapforge/commit/e604fc539ded72abb6052935d6c2876b79cd2fde))
* major enhancements — cross-platform, multi-monitor, history, annotations, tooling ([33f8fe4](https://github.com/biplav00/snapforge/commit/33f8fe40dde4f1fc70d72c604ac82237ad1feca9))
* major enhancements — cross-platform, multi-monitor, history, annotations, tooling ([e366a09](https://github.com/biplav00/snapforge/commit/e366a091f8bf57f14dab481bc2bda7541a6bdf36))
* polish preferences UI — add Recording tab, fix hotkeys, improve styling ([786844b](https://github.com/biplav00/snapforge/commit/786844bc575831dbd228b1edfd1b5e5d3a1ee129))
* register all new tools — update registry, toolbar, canvas, shortcuts ([61f5a9f](https://github.com/biplav00/snapforge/commit/61f5a9fb2bc215617ba469cccc8c4bb93d586f71))
* rename project from ScreenSnap to Snapforge ([d83c526](https://github.com/biplav00/snapforge/commit/d83c5268e9dd858d196018d4755c5408ba3ff849))
* scaffold Cargo workspace with screen-core and cli crates ([b5cb38a](https://github.com/biplav00/snapforge/commit/b5cb38a20e3756524222638b4458947014632b11))
* scaffold preferences window with Vite multi-page build ([02de611](https://github.com/biplav00/snapforge/commit/02de61146cf2eee46b3a4cfb523e0d18f5b5a36c))
* scaffold Svelte + Vite frontend for Tauri ([6988faa](https://github.com/biplav00/snapforge/commit/6988faa34c8d464fd106304cdde89e6c3ae09af4))
* scaffold Tauri v2 app with overlay window and capture commands ([10a802b](https://github.com/biplav00/snapforge/commit/10a802b89fbdc941c2dc60ff582e46d65b1fd43a))
* support bundled FFmpeg — find bundled binary, fall back to system PATH ([81fb78b](https://github.com/biplav00/snapforge/commit/81fb78b3b349a59cba5a8f130217810d42fc2e2b))
* unique app icon — diagonal snap slash with corner brackets ([ca77f11](https://github.com/biplav00/snapforge/commit/ca77f1149650ac99c0e6a9f3be073e6d2ff1823f))


### Bug Fixes

* add serde(default) to config structs for backwards compatibility ([2d1fd28](https://github.com/biplav00/snapforge/commit/2d1fd2803e4abb2b630829739387ca5d6803904b))
* audit fixes — critical, important, and minor issues ([d27d946](https://github.com/biplav00/snapforge/commit/d27d9460b6d46f879596131355e86b5da5c6c190))
* capture screen before overlay window opens ([12f868a](https://github.com/biplav00/snapforge/commit/12f868af0abfbf4c53299411587acfb0138f6641))
* CI clippy on Linux — remove unused Manager import and needless return ([f1fa155](https://github.com/biplav00/snapforge/commit/f1fa1552e0ff895e6459fafe0c8b61ef5f9fee56))
* CI failures + add husky pre-commit, README, lint fixes ([c6a5602](https://github.com/biplav00/snapforge/commit/c6a5602b264a1e1db17cac194719b616e9b96581))
* CI test — display_count may be 0 in headless CI environments ([3f594a8](https://github.com/biplav00/snapforge/commit/3f594a82a89f798259aad2fa3cbb48af380cc1dc))
* CI typecheck — enable allowImportingTsExtensions, fix renderAnnotation type ([347ef36](https://github.com/biplav00/snapforge/commit/347ef36fae296f5e2ae9b2cf8bdbf214fc1dbfd8))
* edge-to-edge overlay without macOS fullscreen Space ([eb30c26](https://github.com/biplav00/snapforge/commit/eb30c2660bb83b555f1cf18be1bc74458d7e897e))
* release-please config — use typed updaters for Cargo.toml and tauri.conf.json ([023e1ed](https://github.com/biplav00/snapforge/commit/023e1ed306cdef3b6285cc5f43595539701ad5d4))
* remove FFmpeg mention from Recording preferences tab ([827cbdc](https://github.com/biplav00/snapforge/commit/827cbdc7661252effae5f25afd77388819d613d0))
* remove ScreenSnap Preferences header text ([854d7ae](https://github.com/biplav00/snapforge/commit/854d7ae605ac811545f18227b04d71c1f3a938a8))
* remove tab icons, show all hotkeys in preferences ([32b5d5c](https://github.com/biplav00/snapforge/commit/32b5d5c0b47e2462e2010794fa483782ce4eb570))
* show app icon in system tray ([d604836](https://github.com/biplav00/snapforge/commit/d604836306656ace680a172cc65ac3f8b7139e9f))
* show copy toast inside region, stay in annotation mode after copy ([a776ebd](https://github.com/biplav00/snapforge/commit/a776ebd4c46f6053ec12372b733b9433f9057305))
* show single colored tray icon instead of two white boxes ([af0d972](https://github.com/biplav00/snapforge/commit/af0d9726a9a25e24fa19e97f4b9ecb47434128dd))
* tray icon — transparent template icon for light/dark macOS themes ([02da838](https://github.com/biplav00/snapforge/commit/02da8384f04db52d923c0d7f67669ce574faf7bb))

## [0.1.0] - 2024-12-01

### Added

- Screen capture with macOS CoreGraphics implementation
- Interactive region selection via Tauri overlay window
- Fullscreen and region screenshot modes
- Screen recording with FFmpeg frame piping (start/stop API)
- Recording indicator UI with red dot, timer, and stop button
- CLI support for screenshots and recording (`--fullscreen`, `--region`)
- Annotation toolkit with multiple tools:
  - Arrow, rectangle, line, and freehand drawing
  - Text annotations
  - Circle, highlight, and step number tools
  - Blur/pixelate and measurement tools
  - Color picker (eyedropper)
- Floating annotation toolbar with color, size, undo/redo, and action controls
- Annotation compositing for save and clipboard copy
- System tray integration with menu actions
- Global hotkeys for screenshot and last-region capture
- Preferences UI with General, Hotkeys, Screenshots, and Recording tabs
- Configurable hotkey bindings via AppConfig
- Custom UI controls (toggle switches, radio dots, styled slider)
- Custom app icon (viewfinder with crosshair and snap bolt)
- Transparent template tray icon for macOS light/dark themes
- Edge-to-edge overlay without macOS fullscreen Space
- Lightshot-style transparent overlay for region selection
- Configuration module with load, save, and filename generation
- Support for multiple capture formats and clipboard output
- Cross-platform Cargo workspace with `screen-core` and `cli` crates
- Svelte + Vite frontend scaffolding for Tauri
- CI/CD pipeline with Biome linting, semantic release, and Dependabot
- Husky pre-commit hooks with lint-staged
