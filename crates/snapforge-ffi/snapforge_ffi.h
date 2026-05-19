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

/* Save RGBA data to file. fmt: 0=PNG, 1=JPG, 2=WebP. Returns 0 on success. */
int snapforge_save_image(const uint8_t *data, uint32_t width, uint32_t height,
                         const char *path, uint32_t fmt, uint8_t quality);

/* Copy RGBA data to clipboard. Returns 0 on success. */
int snapforge_copy_to_clipboard(const uint8_t *data, uint32_t width, uint32_t height);

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

/* --- Recording --- */

/* Start recording. config_json is a JSON string with fields:
   display, region (optional), output_path, format, fps, quality, ffmpeg_path (optional).
   Returns opaque handle on success, NULL on error.
   Caller must call snapforge_stop_recording then snapforge_free_recording_handle. */
void *snapforge_start_recording(const char *config_json);

/* Stop recording and wait for completion. Returns 0 on success, -1 on error. */
int snapforge_stop_recording(void *handle);

/* Check if recording is active. Returns 1 if recording, 0 if not. */
int snapforge_is_recording(void *handle);

/* Free a recording handle. */
void snapforge_free_recording_handle(void *handle);

/* Pause an active recording (output freezes on last frame). Returns 0 on success. */
int snapforge_pause_recording(void *handle);

/* Resume a paused recording. Returns 0 on success. */
int snapforge_resume_recording(void *handle);

/* Last recording error message, or NULL. Caller frees via snapforge_free_string. */
char *snapforge_last_recording_error(void);

/* --- History --- */

/* List history entries as JSON. Caller frees via snapforge_free_string. Returns NULL on error. */
char *snapforge_history_list(void);

/* Add a file path to history.
 * Returns 0 on success, -1 on error, -2 if skipped because the file is an
 * incomplete mp4 (non-fatal; caller may warn). */
int snapforge_history_add(const char *path);

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
 * The wrappers below expose the high-level use cases (snapforge-app) and the
 * Qt frontend has migrated to them in Phase 2C. The primitive
 * snapforge_capture_*, snapforge_*_recording, snapforge_save_image,
 * snapforge_copy_to_clipboard, snapforge_history_add, and
 * snapforge_last_recording_error fns above are deprecated and slated for
 * removal in Phase 2D once all callers are gone. */

/* Last use-case error, or NULL if none. Caller frees via snapforge_free_string.
 * Covers screenshot, recording, and click tracking use cases. */
char *snapforge_app_last_error(void);

/* Take a screenshot end-to-end (capture + save + optional clipboard + optional
 * history). req_json fields:
 *   display (u32), region (optional {x,y,width,height}),
 *   output_path (string), format ("png"/"jpg"/"webp"),
 *   quality (u8 1..=100), copy_to_clipboard (bool), add_to_history (bool).
 * Returns a JSON string {"saved_path": "..."} on success, NULL on error
 * (call snapforge_app_last_error for details). Caller frees via
 * snapforge_free_string. */
char *snapforge_screenshot(const char *req_json);

/* Start recording via the use-case surface. Same JSON as
 * snapforge_start_recording plus add_to_history_on_stop (bool). Returns an
 * opaque handle, or NULL on error (call snapforge_app_last_error). The handle
 * must be stopped via snapforge_record_stop and freed via
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
