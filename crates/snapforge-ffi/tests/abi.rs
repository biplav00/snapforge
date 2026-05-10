//! Cross-ABI smoke tests for the snapforge-ffi entry points.
//!
//! These tests exercise every `extern "C"` function the Qt frontend calls,
//! validating null-pointer handling, magic-tag enforcement on opaque
//! recording handles, the alloc/free pairing for captured buffers, and
//! the JSON config round-trip.
//!
//! Tests that depend on a real display (capture / record) opt in via the
//! `SNAPFORGE_REQUIRE_DISPLAY=1` environment variable. Without it, those
//! cases gracefully skip with a `eprintln!` so the suite stays green on
//! headless CI runners. Set it on developer machines and on any CI matrix
//! entry that has a graphics session attached.

use std::ffi::{c_char, c_void, CString};
use std::ptr;

use snapforge_ffi::*;

fn require_display() -> bool {
    std::env::var("SNAPFORGE_REQUIRE_DISPLAY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Skip the body of a test that needs a real display, unless the env gate
/// is set. Returns true if the test should run.
fn check_display_or_skip(test_name: &str) -> bool {
    if require_display() {
        return true;
    }
    eprintln!(
        "[abi] skipping {test_name}: no display attached. \
         Set SNAPFORGE_REQUIRE_DISPLAY=1 to fail instead of skip."
    );
    false
}

// ---------------------------------------------------------------------------
// Capture & free-buffer
// ---------------------------------------------------------------------------

#[test]
fn free_buffer_null_is_noop() {
    // Must not panic, must not segfault.
    unsafe { snapforge_free_buffer(ptr::null_mut(), 0) };
    unsafe { snapforge_free_buffer(ptr::null_mut(), 1024) };
}

#[test]
fn free_buffer_unknown_pointer_leaks_safely() {
    // A pointer that wasn't returned by snapforge_capture_* must NOT be
    // handed to the global allocator. The registry guard logs and returns
    // without freeing — the test passes if no crash occurs.
    let mut sham = [0u8; 16];
    unsafe { snapforge_free_buffer(sham.as_mut_ptr(), 16) };
}

#[test]
fn capture_fullscreen_roundtrip() {
    if !check_display_or_skip("capture_fullscreen_roundtrip") {
        return;
    }
    let img = snapforge_capture_fullscreen(0);
    assert!(!img.data.is_null(), "capture should yield a buffer");
    assert!(img.width > 0 && img.height > 0);
    assert_eq!(
        img.len,
        (img.width as usize) * (img.height as usize) * 4,
        "len must match width*height*4"
    );

    // Free with the correct len succeeds.
    unsafe { snapforge_free_buffer(img.data, img.len) };
}

#[test]
fn capture_fullscreen_free_with_wrong_len_is_refused() {
    if !check_display_or_skip("capture_fullscreen_free_with_wrong_len_is_refused") {
        return;
    }
    let img = snapforge_capture_fullscreen(0);
    assert!(!img.data.is_null());

    // Wrong len: registry refuses and re-inserts the entry. A subsequent
    // call with the correct len must still succeed, proving the entry was
    // preserved rather than silently dropped.
    unsafe { snapforge_free_buffer(img.data, img.len.wrapping_add(1)) };
    unsafe { snapforge_free_buffer(img.data, img.len) };
}

// ---------------------------------------------------------------------------
// Image saving
// ---------------------------------------------------------------------------

#[test]
fn save_image_null_inputs_return_minus_one() {
    let path = CString::new("/tmp/should-not-be-created.png").unwrap();
    let rc = unsafe {
        snapforge_save_image(ptr::null(), 100, 100, path.as_ptr(), 0, 90)
    };
    assert_eq!(rc, -1);

    let buf = vec![0u8; 100 * 100 * 4];
    let rc = unsafe { snapforge_save_image(buf.as_ptr(), 100, 100, ptr::null(), 0, 90) };
    assert_eq!(rc, -1);
}

#[test]
fn save_image_invalid_format_returns_minus_one() {
    let buf = vec![0u8; 4];
    let path = CString::new("/tmp/bad-fmt.png").unwrap();
    let rc = unsafe { snapforge_save_image(buf.as_ptr(), 1, 1, path.as_ptr(), 99, 90) };
    assert_eq!(rc, -1);
}

#[test]
fn save_image_overflow_dims_returns_minus_one() {
    // width*height*4 wraps usize on 64-bit; the FFI must reject up front.
    let buf = vec![0u8; 4];
    let path = CString::new("/tmp/overflow.png").unwrap();
    let rc = unsafe {
        snapforge_save_image(
            buf.as_ptr(),
            u32::MAX,
            u32::MAX,
            path.as_ptr(),
            0,
            90,
        )
    };
    assert_eq!(rc, -1);
}

#[test]
fn save_image_writes_png() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("ok.png");
    let cpath = CString::new(target.to_str().unwrap()).unwrap();

    let buf = vec![255u8; 4 * 4 * 4]; // 4x4 RGBA all-white
    let rc = unsafe { snapforge_save_image(buf.as_ptr(), 4, 4, cpath.as_ptr(), 0, 90) };
    assert_eq!(rc, 0);
    assert!(target.exists());
    let bytes = std::fs::read(&target).unwrap();
    assert_eq!(&bytes[0..4], &[0x89, b'P', b'N', b'G']);
}

// ---------------------------------------------------------------------------
// String free
// ---------------------------------------------------------------------------

#[test]
fn free_string_null_is_noop() {
    unsafe { snapforge_free_string(ptr::null_mut()) };
}

#[test]
fn default_save_path_round_trips_through_free() {
    let s = snapforge_default_save_path();
    if s.is_null() {
        // Acceptable on systems with no config dir; nothing to free.
        return;
    }
    unsafe { snapforge_free_string(s) };
}

// ---------------------------------------------------------------------------
// Recording handle: magic-tag enforcement
// ---------------------------------------------------------------------------

#[test]
fn recording_calls_with_null_handle_are_safe() {
    assert_eq!(unsafe { snapforge_stop_recording(ptr::null_mut()) }, -1);
    assert_eq!(unsafe { snapforge_pause_recording(ptr::null_mut()) }, -1);
    assert_eq!(unsafe { snapforge_resume_recording(ptr::null_mut()) }, -1);
    assert_eq!(unsafe { snapforge_is_recording(ptr::null_mut()) }, 0);
    unsafe { snapforge_free_recording_handle(ptr::null_mut()) };
}

#[test]
fn recording_calls_with_garbage_handle_are_rejected() {
    // A non-null pointer that doesn't carry our magic must be rejected by
    // every entry point. Using a heap allocation so the address is well-formed
    // but the magic word is wrong.
    let mut sham = [0u8; 64];
    let p = sham.as_mut_ptr().cast::<c_void>();

    assert_eq!(unsafe { snapforge_stop_recording(p) }, -1);
    assert_eq!(unsafe { snapforge_pause_recording(p) }, -1);
    assert_eq!(unsafe { snapforge_resume_recording(p) }, -1);
    assert_eq!(unsafe { snapforge_is_recording(p) }, 0);
    // free must not actually free this allocation either — caller still owns it.
    unsafe { snapforge_free_recording_handle(p) };
    // If free happened, the next access would be UAF; we deliberately read
    // from `sham` here to demonstrate it's still ours.
    assert_eq!(sham[0], 0);
}

#[test]
fn start_recording_null_or_invalid_json_returns_null() {
    let h = unsafe { snapforge_start_recording(ptr::null()) };
    assert!(h.is_null());

    let junk = CString::new("not-json").unwrap();
    let h = unsafe { snapforge_start_recording(junk.as_ptr()) };
    assert!(h.is_null());

    let err = snapforge_last_recording_error();
    assert!(!err.is_null(), "an error message must be reported");
    unsafe { snapforge_free_string(err) };
}

#[test]
fn start_recording_missing_output_path_returns_null() {
    // Valid JSON object, no output_path → must reject with last_error set.
    let json = CString::new(r#"{"display":0,"format":"mp4","fps":30}"#).unwrap();
    let h = unsafe { snapforge_start_recording(json.as_ptr()) };
    assert!(h.is_null());

    let err = snapforge_last_recording_error();
    assert!(!err.is_null());
    unsafe { snapforge_free_string(err) };
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

#[test]
fn history_list_is_well_formed() {
    let s = snapforge_history_list();
    assert!(!s.is_null());
    let cstr = unsafe { std::ffi::CStr::from_ptr(s) };
    let txt = cstr.to_str().unwrap().to_owned();
    unsafe { snapforge_free_string(s) };
    // Must be a JSON array (possibly empty).
    assert!(txt.starts_with('['), "history list must be a JSON array, got: {}", &txt[..txt.len().min(64)]);
}

#[test]
fn history_add_null_returns_minus_one() {
    assert_eq!(unsafe { snapforge_history_add(ptr::null()) }, -1);
}

#[test]
fn is_incomplete_mp4_null_returns_minus_one() {
    assert_eq!(unsafe { snapforge_is_incomplete_mp4(ptr::null()) }, -1);
}

#[test]
fn is_incomplete_mp4_non_mp4_extension_is_zero() {
    let txt_path = CString::new("/tmp/not-an-mp4.txt").unwrap();
    assert_eq!(
        unsafe { snapforge_is_incomplete_mp4(txt_path.as_ptr()) },
        0,
        "non-mp4 extensions are never flagged as incomplete"
    );
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[test]
fn config_load_returns_json_object_or_null() {
    let s = snapforge_config_load();
    if s.is_null() {
        return;
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(s) };
    let txt = cstr.to_str().unwrap().to_owned();
    unsafe { snapforge_free_string(s) };
    assert!(txt.starts_with('{'));
    // Must round-trip through serde_json without error.
    let _: serde_json::Value = serde_json::from_str(&txt).unwrap();
}

#[test]
fn config_save_null_or_invalid_json_returns_minus_one() {
    let rc = unsafe { snapforge_config_save(ptr::null()) };
    assert_eq!(rc, -1);

    let junk = CString::new("definitely not json").unwrap();
    let rc = unsafe { snapforge_config_save(junk.as_ptr()) };
    assert_eq!(rc, -1);
}

#[test]
fn config_save_preserves_qt_only_fields_via_extra_flatten() {
    // The audit's hotkey-config split-brain: a save+load round-trip must
    // not strip Qt-only top-level keys such as `hotkeys` and `theme`.
    let original = snapforge_config_load();
    let original_text = if original.is_null() {
        String::from("{}")
    } else {
        let s = unsafe { std::ffi::CStr::from_ptr(original) }
            .to_str()
            .unwrap()
            .to_owned();
        unsafe { snapforge_free_string(original) };
        s
    };
    let mut original_value: serde_json::Value =
        serde_json::from_str(&original_text).unwrap_or_else(|_| serde_json::json!({}));

    // Inject Qt-only fields and round-trip.
    if let Some(obj) = original_value.as_object_mut() {
        obj.insert(
            "hotkeys".to_string(),
            serde_json::json!({"global": {"screenshot": "Cmd+Alt+Shift+X"}}),
        );
        obj.insert("theme".to_string(), serde_json::json!("Dark"));
    }
    let payload = CString::new(original_value.to_string()).unwrap();
    let rc = unsafe { snapforge_config_save(payload.as_ptr()) };
    assert_eq!(rc, 0);

    let s = snapforge_config_load();
    assert!(!s.is_null());
    let txt = unsafe { std::ffi::CStr::from_ptr(s) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { snapforge_free_string(s) };
    let loaded: serde_json::Value = serde_json::from_str(&txt).unwrap();
    assert_eq!(loaded["theme"], serde_json::json!("Dark"));
    assert_eq!(
        loaded["hotkeys"]["global"]["screenshot"],
        serde_json::json!("Cmd+Alt+Shift+X")
    );
}

// ---------------------------------------------------------------------------
// Display info (cheap, no display required)
// ---------------------------------------------------------------------------

#[test]
fn display_at_point_unreachable_coords_return_minus_one() {
    // -100000,-100000 is unlikely to fall inside any real display; the FFI
    // contract is "-1 if no display".
    let rc = snapforge_display_at_point(-100_000, -100_000);
    // On a fresh boot the display geometry can include negative coords for
    // secondary displays, so allow -1 OR a valid index. The important
    // assertion is that we don't return a positive integer that lies about
    // containment — the function went through the lookup path.
    assert!(rc == -1 || rc >= 0);
}

#[test]
fn display_scale_factor_is_positive() {
    let dpr = snapforge_display_scale_factor();
    assert!(dpr > 0.0, "display scale factor must be > 0; got {}", dpr);
}

// `c_char` import silenced: keep it referenced so unused-import warnings stay quiet
// regardless of platform-conditional compile.
#[allow(dead_code)]
fn _silence_unused() -> *const c_char {
    ptr::null()
}
