# FFI boundary

Source: `crates/snapforge-ffi/src/lib.rs`. C consumer: `qt/src/*` via `extern "C"` declarations from `crates/snapforge-ffi/snapforge_ffi.h` (hand-maintained — keep in sync with `src/lib.rs` when adding fns).

## Surface split

Two surfaces ship side by side as of Phase 2C:

- **Use-case FFI** (`snapforge_screenshot`, `snapforge_record_*`, `snapforge_clicks_*`, `snapforge_app_last_error`). Wraps the high-level workflows in `crates/snapforge-app`. **This is the surface the Qt frontend uses going forward.** The Qt recording + click paths have been migrated; the screenshot save path still uses primitives (see below).
- **Primitive FFI** (`snapforge_capture_*`, `snapforge_save_image`, `snapforge_copy_to_clipboard`, `snapforge_start/stop/pause/resume_recording`, `snapforge_is_recording`, `snapforge_last_recording_error`, `snapforge_free_recording_handle`). Deprecated. Slated for removal in **Phase 2D** once every Qt callsite is migrated or the missing use-case surface is filled in. Until 2D ships, **do not add new callers of the primitives**.

### Phase 2C screenshot exception

`snapforge_screenshot` re-captures internally — it cannot accept Qt-rendered pixel data. The Qt save path composites annotations on top of the captured backdrop before writing, so `snapforge_save_image` / `snapforge_copy_to_clipboard` / `snapforge_history_add` still ship the bytes that Qt has already prepared. Phase 2D will either (a) add a `save_prerendered` use-case, or (b) move annotation rendering into a Rust crate, before removing those three primitives. The dim-backdrop capture in `OverlayWindow::activateInternal` and the Cmd+C region-to-clipboard path are also blocked until that decision lands. Everything else on the primitive surface (recording lifecycle, clicks, error string) has a Qt-side replacement and is safe to delete in 2D.

## Conventions

- **Return code**: `0` = success, `-1` = error (where the fn returns `c_int`).
- **Pointer errors**: `NULL` returned. Caller must check before dereference.
- **Error message retrieval**: recording-family errors stash a string in `LAST_RECORDING_ERROR`. Read once via `snapforge_last_recording_error()`; the returned string is owned by the caller.
- **Owned buffers**: `*mut u8 + len`. Free with `snapforge_free_buffer(ptr, len)`. **Length must match** — mismatched length = heap corruption (registry catches it and aborts).
- **Owned strings**: `*mut c_char`. Free with `snapforge_free_string(ptr)`.
- **Opaque handles**: recording = `*mut c_void`. Free with `snapforge_free_recording_handle(h)`. Pointer must be in `HANDLE_REGISTRY` or it is rejected.
- **NUL safety**: input strings with embedded NUL are sanitized + logged (`cstring_sanitized`).

## Function surface

### Image capture

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_capture_fullscreen(display: u32)` | `CapturedImage` (RGBA buf + dims) | On failure all fields zeroed |
| `snapforge_capture_region(display, x, y, w, h)` | `CapturedImage` | Same error contract |
| `snapforge_free_buffer(ptr, len)` | void | **len must equal returned len** |

### Image save / clipboard

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_save_image(ptr, len, w, h, path, fmt, quality)` | `c_int` | fmt = 0 PNG / 1 JPG / 2 WebP |
| `snapforge_copy_to_clipboard(ptr, len, w, h)` | `c_int` | RGBA, no premultiplied alpha |

### Permissions / display

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_has_permission()` | `c_int` (1/0) | TCC Screen Recording grant |
| `snapforge_request_permission()` | `c_int` | Triggers the OS prompt |
| `snapforge_display_at_point(x, y)` | `c_int` | Returns display index or -1 |
| `snapforge_display_scale_factor()` | `f64` | DPR of primary display |
| `snapforge_default_save_path()` | `*mut c_char` | `~/Pictures/Snapforge` or platform equivalent. Free with `snapforge_free_string` |
| `snapforge_free_string(ptr)` | void | |

### Recording

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_start_recording(config_json)` | `*mut c_void` (handle) | JSON keys: `display, region{x,y,width,height}, output_path, format, fps, quality, ffmpeg_path`. NULL on error → check `snapforge_last_recording_error` |
| `snapforge_stop_recording(handle)` | `c_int` | Waits for ffmpeg flush |
| `snapforge_pause_recording(handle)` | `c_int` | |
| `snapforge_resume_recording(handle)` | `c_int` | |
| `snapforge_is_recording(handle)` | `c_int` (1/0) | |
| `snapforge_last_recording_error()` | `*mut c_char` | Owned. Reading does not clear — successful start_recording does. |
| `snapforge_free_recording_handle(handle)` | void | Must be called even after stop |

### History

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_history_list()` | `*mut c_char` | JSON array string |
| `snapforge_history_add(path)` | `c_int` | |
| `snapforge_history_delete(path)` | `c_int` | |
| `snapforge_history_clear()` | `c_int` | |
| `snapforge_is_incomplete_mp4(path)` | `c_int` (1/0) | Used by history view to flag dead recordings |

### Config

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_config_load()` | `*mut c_char` | JSON object string. Free with `snapforge_free_string` |
| `snapforge_config_save(json)` | `c_int` | Whole-object replace (no merge) |

### Use-case surface (snapforge-app)

These wrap the high-level workflows. Errors are surfaced via a single shared `snapforge_app_last_error()` instead of one error fn per domain.

| Function | Returns | Notes |
|----------|---------|-------|
| `snapforge_screenshot(req_json)` | `*mut c_char` | JSON `{"saved_path":"..."}` on success; NULL on error. Request JSON: `display, region{x,y,w,h}?, output_path, format, quality, copy_to_clipboard, add_to_history`. Currently unused by Qt — see "Phase 2C screenshot exception" above. |
| `snapforge_record_start(req_json)` | `*mut c_void` (handle) | Same JSON as `snapforge_start_recording` plus `add_to_history_on_stop`. Indexes the finished file on successful stop. |
| `snapforge_record_stop(handle)` | `c_int` | |
| `snapforge_record_pause(handle)` | `c_int` | |
| `snapforge_record_resume(handle)` | `c_int` | |
| `snapforge_record_free_handle(handle)` | void | |
| `snapforge_clicks_start(cb, user_data)` | `*mut c_void` (handle) | `cb` fires on a Rust-owned thread; Qt must dispatch to its main thread (see `ClickTap::onClickStatic`). NULL on failure (typically missing Input Monitoring grant). |
| `snapforge_clicks_stop(handle)` | `c_int` | |
| `snapforge_clicks_free_handle(handle)` | void | |
| `snapforge_app_last_error()` | `*mut c_char` | Shared error string for screenshot / record / clicks. Owned. |

## Lifetime ownership table

| Object | Allocator | Freer | Registry |
|--------|-----------|-------|----------|
| `CapturedImage.data` | Rust | Qt → `snapforge_free_buffer` | `BUFFER_REGISTRY` |
| Strings returned by FFI | Rust | Qt → `snapforge_free_string` | none — caller must not mismatch |
| Recording handle | Rust | Qt → `snapforge_free_recording_handle` | `HANDLE_REGISTRY` (rejects unknown) |
| JSON inputs | Qt | Qt (Rust copies) | none |

## Error contracts at a glance

```
Capture failure:   CapturedImage with data=NULL, len=0, w=0, h=0
Save/copy failure: -1 (no detail surfaced; check Rust logs)
Recording failure: NULL handle (start) or -1 (others); detail in snapforge_last_recording_error()
Config failure:    NULL (load) or -1 (save)
```

## When adding a new FFI fn

1. Add `#[no_mangle] pub extern "C" fn snapforge_<name>(...)` in `crates/snapforge-ffi/src/lib.rs`.
2. Regenerate header (cbindgen) if used, or declare in the Qt source manually.
3. Match the conventions above — return codes, ownership, registry tracking.
4. Add a smoke test in `crates/snapforge-ffi/tests/abi.rs`.
5. Update this doc's tables.
