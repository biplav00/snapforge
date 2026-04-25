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

#ifdef __cplusplus
}
#endif

#endif /* SNAPFORGE_FFI_H */
