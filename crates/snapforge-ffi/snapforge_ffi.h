#ifndef SNAPFORGE_FFI_H
#define SNAPFORGE_FFI_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * CapturedImage error contract:
 *   On success: data != NULL, len > 0, width > 0, height > 0.
 *   On failure: data == NULL, len == 0, width == 0, height == 0.
 * Callers must check (data != NULL && width > 0 && height > 0) before use.
 */
typedef struct {
    uint8_t *data;
    size_t len;
    uint32_t width;
    uint32_t height;
} CapturedImage;

/* Capture fullscreen. Caller must free result.data via snapforge_free_buffer.
 * On failure all fields of the returned struct are zero (see CapturedImage). */
CapturedImage snapforge_capture_fullscreen(uint32_t display);

/* Capture a region. Caller must free result.data via snapforge_free_buffer.
 * On failure all fields of the returned struct are zero (see CapturedImage). */
CapturedImage snapforge_capture_region(uint32_t display, int32_t x, int32_t y, uint32_t w, uint32_t h);

/* Free a pixel buffer returned by snapforge_capture_*. */
void snapforge_free_buffer(uint8_t *data, size_t len);

/* Check screen capture permission. Returns 1 if granted. */
int snapforge_has_permission(void);

/* Request screen capture permission. Returns 1 if granted. */
int snapforge_request_permission(void);

/* Find display index for a point (in screen coordinates). Returns display index or -1. */
int snapforge_display_at_point(int32_t x, int32_t y);

/* Get primary display DPI scale factor. */
double snapforge_display_scale_factor(void);

/* Get default save directory. Caller must free via snapforge_free_string. */
char *snapforge_default_save_path(void);

/* Free a string returned by snapforge_*. */
void snapforge_free_string(char *s);

/* --- History --- */

/* List history entries as JSON. Caller frees via snapforge_free_string. Returns NULL on error. */
char *snapforge_history_list(void);

/* Delete a history entry by path. Returns 0 on success, -1 on error. */
int snapforge_history_delete(const char *path);

/* Clear all history. Returns 0 on success, -1 on error. */
int snapforge_history_clear(void);

/* Returns 1 if the given path is an incomplete mp4 (zero-byte or no moov atom),
 * 0 otherwise. Returns -1 on null/invalid input. Non-mp4 paths return 0. */
int snapforge_is_incomplete_mp4(const char *path);

/* --- Config --- */

/* Load config as JSON. Caller frees via snapforge_free_string. Returns NULL on error. */
char *snapforge_config_load(void);

/* Save config from JSON. Returns 0 on success, -1 on error. */
int snapforge_config_save(const char *json);

/* --- Use-case FFI (snapforge-app)
 *
 * The wrappers below expose the high-level use cases (snapforge-app) which
 * are the only sanctioned entry points for new callers. The remaining
 * primitives above (snapforge_capture_*, snapforge_free_buffer,
 * snapforge_has/request_permission, snapforge_display_*,
 * snapforge_default_save_path, snapforge_history_list/delete/clear,
 * snapforge_is_incomplete_mp4, snapforge_config_*, snapforge_free_string)
 * are kept because they are either raw building blocks the use-case layer
 * cannot subsume (capture, free) or read-only metadata helpers. */

/* Last use-case error, or NULL if none. Caller frees via snapforge_free_string.
 * Covers screenshot, recording, and click tracking use cases. */
char *snapforge_app_last_error(void);

/* Stable category of the last use-case error, alongside the human-readable
 * string from snapforge_app_last_error. Values mirror Rust's
 * SnapforgeErrorCode; only ever appended to, never renumbered. Returns
 * SNAPFORGE_ERR_NONE (0) when the last call succeeded, or
 * SNAPFORGE_ERR_INTERNAL if the internal error-state lock is poisoned. */
typedef enum {
    SNAPFORGE_ERR_NONE              = 0,
    SNAPFORGE_ERR_INVALID_INPUT     = 1,
    SNAPFORGE_ERR_PERMISSION_DENIED = 2,
    SNAPFORGE_ERR_NOT_FOUND         = 3,
    SNAPFORGE_ERR_IO                = 4,
    SNAPFORGE_ERR_ENCODE            = 5,
    SNAPFORGE_ERR_CAPTURE           = 6,
    SNAPFORGE_ERR_CONFIG            = 7,
    SNAPFORGE_ERR_INTERNAL          = 8
} SnapforgeErrorCode;

int snapforge_app_last_error_code(void);

/* --- Logging bridge --- */

/* Severity passed to a SnapforgeLogCallback; mirrors Rust's SnapforgeLogLevel
 * and lines up with severity so callers can map onto their own log levels. */
typedef enum {
    SNAPFORGE_LOG_TRACE = 0,
    SNAPFORGE_LOG_DEBUG = 1,
    SNAPFORGE_LOG_INFO  = 2,
    SNAPFORGE_LOG_WARN  = 3,
    SNAPFORGE_LOG_ERROR = 4
} SnapforgeLogLevel;

/* Callback invoked for every Rust log record once registered via
 * snapforge_set_log_callback. `level` is a SnapforgeLogLevel; `msg` is a
 * NUL-terminated UTF-8 string owned by Rust and valid ONLY for the duration of
 * the call — copy it, do not retain the pointer. May fire from arbitrary Rust
 * threads, so the callback must be thread-safe. It must not throw/unwind. */
typedef void (*SnapforgeLogCallback)(int level, const char *msg);

/* Register the callback that receives formatted Rust log records and install
 * the logging subscriber (once). Call once at startup to route Rust
 * diagnostics into the host log; until then Rust logs go to stderr. Calling
 * again replaces the callback. `callback` must stay valid for the process
 * lifetime. */
void snapforge_set_log_callback(SnapforgeLogCallback callback);

/* Take a screenshot end-to-end (capture + save + optional clipboard + optional
 * history). req_json fields:
 *   display (u32), region (optional {x,y,width,height}),
 *   output_path (string), format ("png"/"jpg"/"webp"),
 *   quality (u8 1..=100), copy_to_clipboard (bool), add_to_history (bool).
 * Returns a JSON string {"saved_path": "..."} on success, NULL on error
 * (call snapforge_app_last_error for details). Caller frees via
 * snapforge_free_string. */
char *snapforge_screenshot(const char *req_json);

/* Save (and optionally clipboard / index) a caller-supplied RGBA bitmap.
 *
 * `rgba` points to `rgba_len` bytes of RGBA8 pixels; `rgba_len` must equal
 * width*height*4. The buffer is read-only and ownership stays with the
 * caller (Rust copies it internally).
 *
 * req_json fields:
 *   output_path (optional string; omit/empty for clipboard-only),
 *   format ("png"/"jpg"/"webp", default "png"; ignored when no path),
 *   quality (u8 1..=100, default 90),
 *   copy_to_clipboard (bool, default false),
 *   add_to_history (bool, default false; ignored when no path).
 *
 * Returns a JSON string {"saved_path": "..." | null} on success, NULL on
 * error (call snapforge_app_last_error). Caller frees via
 * snapforge_free_string. */
char *snapforge_save_prerendered(const uint8_t *rgba, size_t rgba_len,
                                 uint32_t width, uint32_t height,
                                 const char *req_json);

/* Start recording via the use-case surface.
 *
 * req_json fields (all optional; missing fields take defaults):
 *   display (uint, default 0), region (optional {x,y,width,height}),
 *   output_path (string; empty is rejected), format ("mp4"/"gif", default
 *   "mp4"), fps (u32, default 30, clamped 1..=240), quality
 *   ("low"/"medium"/"high", default "medium"), ffmpeg_path (optional string),
 *   add_to_history_on_stop (bool, default false),
 *   show_clicks (bool, default false).
 *
 * Returns an opaque handle, or NULL on error (call snapforge_app_last_error).
 * The handle must be stopped via snapforge_record_stop and freed via
 * snapforge_record_free_handle. */
void *snapforge_record_start(const char *req_json);

/* Stop a use-case recording. Returns 0 on success, -1 on error
 * (call snapforge_app_last_error for details). */
int snapforge_record_stop(void *handle);

/* Pause a use-case recording. Returns 0 on success, -1 on error. */
int snapforge_record_pause(void *handle);

/* Resume a use-case recording. Returns 0 on success, -1 on error. */
int snapforge_record_resume(void *handle);

/* Free a use-case recording handle. Drops the inner handle if still active. */
void snapforge_record_free_handle(void *handle);

/* Callback invoked for every global click event by snapforge_clicks_start.
 * Fires on a Rust-owned thread (NOT the caller's main thread); callers must
 * dispatch back to their UI thread themselves. right_click is 1 for
 * right-mouse-down, 0 for left-mouse-down. user_data is the opaque pointer
 * supplied to snapforge_clicks_start; Rust never dereferences it. */
typedef void (*SnapforgeClickCallback)(double x, double y, int right_click,
                                       void *user_data);

/* Begin streaming global click events to `callback`. Returns an opaque handle
 * or NULL on failure (typically missing Accessibility / Input Monitoring
 * permission; call snapforge_app_last_error for details). The handle must be
 * stopped via snapforge_clicks_stop and freed via snapforge_clicks_free_handle.
 *
 * `callback` must remain valid for the lifetime of the returned handle.
 * `user_data` must remain valid for the same lifetime if the callback
 * dereferences it. */
void *snapforge_clicks_start(SnapforgeClickCallback callback, void *user_data);

/* Stop streaming click events. The handle allocation stays valid until
 * snapforge_clicks_free_handle. Returns 0 on success, -1 on error. */
int snapforge_clicks_stop(void *handle);

/* Free a click handle. Stops the tap if still active. */
void snapforge_clicks_free_handle(void *handle);

#ifdef __cplusplus
}
#endif

#endif /* SNAPFORGE_FFI_H */
