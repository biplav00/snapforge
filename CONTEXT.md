# Snapforge — domain & architecture glossary

Names for the load-bearing concepts in the codebase. Keep entries terse; this
is a shared vocabulary, not documentation. Architecture terms (module, seam,
adapter, depth) are used in the `/codebase-design` sense.

## FFI seam

**Use-case FFI** — the sanctioned `snapforge_*` entry points that wrap the
high-level workflows in `snapforge-app` (screenshot, save-prerendered, record,
clicks). Take a JSON request, return a JSON string or opaque handle, surface
errors through the single `snapforge_app_last_error()`. See
`docs/architecture/ffi-boundary.md`.

**Request DTO** — the canonical serde-`Deserialize` structs in `snapforge-app`
(`ScreenshotRequest`, `SavePrerenderedRequest`, `RecordingRequest`) that define
every request field, default, and enum spelling exactly once. The FFI
deserializes JSON straight into them; nothing hand-parses fields.

**SnapforgeClient** — the Qt-side adapter that owns the FFI seam from the
frontend's side: the single translation unit that `#include`s
`snapforge_ffi.h`. It hides JSON assembly, `snapforge_app_last_error()`
retrieval, and string/buffer freeing behind typed Qt calls, so no widget,
window, or controller touches `snapforge_*` directly. The seam is *real*
because it has two adapters: `SnapforgeClient.cpp` (calls the real FFI) and
`SnapforgeClientFake.cpp` (in-memory), swapped at **link time** — the app links
the real one, each test target links the fake and omits `libsnapforge_ffi.a`.
Per-test behaviour lives on the fake's settable state (`sf::test::*`), e.g.
`fireClick` to simulate a system click without an Input Monitoring grant.
