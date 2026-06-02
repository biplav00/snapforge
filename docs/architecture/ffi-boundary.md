# FFI boundary

Source: `crates/snapforge-ffi/src/lib.rs`. C consumer: `qt/src/*` via `extern "C"` declarations from `crates/snapforge-ffi/snapforge_ffi.h` (hand-maintained — keep in sync with `src/lib.rs` when adding fns).

## Surface split

As of Phase 2D the FFI is split into two cleanly-scoped surfaces:

- **Use-case FFI** (`snapforge_screenshot`, `snapforge_save_prerendered`, `snapforge_record_*`, `snapforge_clicks_*`, `snapforge_app_last_error`). Wraps the high-level workflows in `crates/snapforge-app`. **All Qt callsites use this surface.** Errors flow through a single `snapforge_app_last_error()`.
- **Primitive surface** that remains: raw screen capture (`snapforge_capture_fullscreen` / `_region` / `_free_buffer`), TCC checks, display / path metadata, history read/write that isn't subsumed by add, config, and the universal `snapforge_free_string`. These are kept because they're either raw building blocks the use-case layer cannot subsume (capture, free) or read-only metadata helpers that don't fit the "use case" shape.

### What was removed in Phase 2D

`snapforge_save_image`, `snapforge_copy_to_clipboard`, `snapforge_history_add`, and the recording-primitive set (`snapforge_start/stop/pause/resume_recording`, `snapforge_is_recording`, `snapforge_free_recording_handle`, `snapforge_last_recording_error`). All callers now use `snapforge_save_prerendered` and `snapforge_record_*`.

### Why `snapforge_save_prerendered` exists

`snapforge_screenshot` captures the bitmap inside Rust before saving — that's fine for fullscreen / region grabs but not for the annotated screenshot flow, where Qt composites overlays on top of the captured backdrop before the save. Re-capturing inside Rust would drop the overlays. `snapforge_save_prerendered` accepts the already-composited RGBA bytes verbatim and handles the encode / clipboard / history-index tail. Same use case covers the Cmd+C region-to-clipboard path with `output_path` omitted.

## Conventions

- **Return code**: `0` = success, `-1` = error (where the fn returns `c_int`).
- **Pointer errors**: `NULL` returned. Caller must check before dereference.
- **Error message retrieval**: every use-case fn writes a string to `LAST_APP_ERROR` on failure. Read via `snapforge_app_last_error()`; the returned string is owned by the caller. Reading does not clear; a subsequent successful call does.
- **Owned buffers**: `*mut u8 + len`. Free with `snapforge_free_buffer(ptr, len)`. **Length must match** — mismatched length = heap corruption (registry catches it and aborts).
- **Owned strings**: `*mut c_char`. Free with `snapforge_free_string(ptr)`.
- **Opaque handles**: recording / click handles = `*mut c_void`. Free with `snapforge_record_free_handle` / `snapforge_clicks_free_handle`. Pointer must be in `HANDLE_REGISTRY` or it is rejected.
- **NUL safety**: input strings with embedded NUL are sanitized + logged (`cstring_sanitized`).

## Function surface

### Image capture (primitive)

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_capture_fullscreen(display: u32)` | `CapturedImage` (RGBA buf + dims) | On failure all fields zeroed |
| `snapforge_capture_region(display, x, y, w, h)` | `CapturedImage` | Same error contract |
| `snapforge_free_buffer(ptr, len)` | void | **len must equal returned len** |

These remain because they are the only way to obtain a raw desktop bitmap that the caller can then annotate / composite before handing back to `snapforge_save_prerendered`.

### Permissions / display / paths (primitive)

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_has_permission()` | `c_int` (1/0) | TCC Screen Recording grant |
| `snapforge_request_permission()` | `c_int` | Triggers the OS prompt |
| `snapforge_display_at_point(x, y)` | `c_int` | Returns display index or -1 |
| `snapforge_display_scale_factor()` | `f64` | DPR of primary display |
| `snapforge_default_save_path()` | `*mut c_char` | `~/Pictures/Snapforge` or platform equivalent. Free with `snapforge_free_string` |
| `snapforge_free_string(ptr)` | void | Universal string free |

### History (primitive)

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_history_list()` | `*mut c_char` | JSON array string |
| `snapforge_history_delete(path)` | `c_int` | |
| `snapforge_history_clear()` | `c_int` | |
| `snapforge_is_incomplete_mp4(path)` | `c_int` (1/0) | Used by history view to flag dead recordings |

`snapforge_history_add` was removed; indexing is now handled inside `snapforge_save_prerendered` (via `add_to_history`) and `snapforge_record_start` (via `add_to_history_on_stop`).

### Config (primitive)

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_config_load()` | `*mut c_char` | JSON object string. Free with `snapforge_free_string` |
| `snapforge_config_save(json)` | `c_int` | Whole-object replace (no merge) |

### Use-case surface (snapforge-app)

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_screenshot(req_json)` | `*mut c_char` | JSON `{"saved_path":"..."}` on success; NULL on error. Request JSON: `display, region{x,y,w,h}?, output_path, format, quality, copy_to_clipboard, add_to_history`. Captures internally. |
| `snapforge_save_prerendered(rgba, rgba_len, w, h, req_json)` | `*mut c_char` | JSON `{"saved_path":"..."\|null}` on success. Request JSON: `output_path?, format?, quality?, copy_to_clipboard?, add_to_history?`. With `output_path` omitted = clipboard-only. Buffer is read-only; caller retains ownership. |
| `snapforge_record_start(req_json)` | `*mut c_void` (handle) | JSON: `display, region?, output_path, format, fps, quality, ffmpeg_path?, add_to_history_on_stop`. Indexes the finished file on successful stop. |
| `snapforge_record_stop(handle)` | `c_int` | |
| `snapforge_record_pause(handle)` | `c_int` | |
| `snapforge_record_resume(handle)` | `c_int` | |
| `snapforge_record_free_handle(handle)` | void | |
| `snapforge_clicks_start(cb, user_data)` | `*mut c_void` (handle) | `cb` fires on a Rust-owned thread; Qt must dispatch to its main thread (see `ClickTap::onClickStatic`). NULL on failure (typically missing Input Monitoring grant). |
| `snapforge_clicks_stop(handle)` | `c_int` | |
| `snapforge_clicks_free_handle(handle)` | void | |
| `snapforge_app_last_error()` | `*mut c_char` | Shared error string for every use case. Owned. Poisoned lock surfaces a synthetic "lock poisoned" string, not NULL. |
| `snapforge_app_last_error_code()` | `c_int` | Stable `SnapforgeErrorCode` category of the last error (0 = none). Poisoned lock returns `Internal` (8), never a misleading 0. |

### Logging bridge

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_set_log_callback(cb)` | void | Registers a `SnapforgeLogCallback(int level, const char *msg)` that receives formatted Rust `tracing` records, and installs the subscriber once. `level` is a `SnapforgeLogLevel`; `msg` is Rust-owned and valid only for the call (copy it). Fires from arbitrary Rust threads; callback must be thread-safe and must not unwind. With no callback registered, Rust logs go to stderr. |

`SnapforgeErrorCode` (repr i32, append-only): `None=0, InvalidInput=1, PermissionDenied=2, NotFound=3, Io=4, Encode=5, Capture=6, Config=7, Internal=8`. Derived from `AppError`/sub-crate error variants.

## Lifetime ownership table

| Object | Allocator | Freer | Registry |
|--------|-----------|-------|----------|
| `CapturedImage.data` | Rust | Qt → `snapforge_free_buffer` | `BUFFER_REGISTRY` |
| Strings returned by FFI | Rust | Qt → `snapforge_free_string` | none — caller must not mismatch |
| Recording handle | Rust | Qt → `snapforge_record_free_handle` | `HANDLE_REGISTRY` (rejects unknown) |
| Click handle | Rust | Qt → `snapforge_clicks_free_handle` | `HANDLE_REGISTRY` |
| JSON inputs | Qt | Qt (Rust copies) | none |
| `rgba` to `snapforge_save_prerendered` | Qt | Qt (Rust copies internally) | none |

## Error contracts at a glance

```
Capture failure:    CapturedImage with data=NULL, len=0, w=0, h=0
Use-case failure:   NULL (string return) or -1 (c_int return); detail in snapforge_app_last_error()
Config failure:     NULL (load) or -1 (save)
```

## When adding a new FFI fn

1. Add `#[no_mangle] pub extern "C" fn snapforge_<name>(...)` in `crates/snapforge-ffi/src/lib.rs`.
2. Mirror the declaration in `crates/snapforge-ffi/snapforge_ffi.h` (hand-maintained).
3. Prefer the use-case surface: take a JSON request, return a JSON string or opaque handle, surface errors via `snapforge_app_last_error()`. Add the underlying logic to `crates/snapforge-app`.
4. Match the conventions above — registry tracking for any opaque handle, NUL-safe strings, length-checked buffers.
5. Add a smoke test in `crates/snapforge-ffi/tests/abi.rs`.
6. Update this doc's tables.
