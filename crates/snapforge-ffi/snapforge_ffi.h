#ifndef SNAPFORGE_FFI_H
#define SNAPFORGE_FFI_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint8_t *data;
    size_t len;
    uint32_t width;
    uint32_t height;
} CapturedImage;

/* Capture fullscreen. Caller must free result.data via snapforge_free_buffer. */
CapturedImage snapforge_capture_fullscreen(uint32_t display);

/* Capture a region. Caller must free result.data via snapforge_free_buffer. */
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

/* Get number of displays. */
uint32_t snapforge_display_count(void);

/* Get primary display DPI scale factor. */
double snapforge_display_scale_factor(void);

/* Get default save directory. Caller must free via snapforge_free_string. */
char *snapforge_default_save_path(void);

/* Free a string returned by snapforge_*. */
void snapforge_free_string(char *s);

#ifdef __cplusplus
}
#endif

#endif /* SNAPFORGE_FFI_H */
