#ifndef HOTKEYKEYMAP_H
#define HOTKEYKEYMAP_H

// Shared by the Preferences hotkey recorder and the overlay's key matcher.
//
// On macOS Qt reports QKeyEvent::key() from the *shifted* character, so
// Shift+1 arrives as Qt::Key_Exclam and Shift+/ as Qt::Key_Question. A chord
// recorded that way ("Shift+!") can't be parsed back into a Carbon virtual key
// (the global-hotkey path is keycode based) and reads badly in the badge. Both
// the recorder and the overlay therefore normalise to the UNSHIFTED key: first
// from the hardware virtual keycode (kVK_ANSI_* positions, macOS only), then by
// mapping the US-layout shifted symbol back to its base key.

#include <Qt>
#include <QtGlobal>

#include "Shortcuts.h"

namespace hotkeykeymap {

inline int unshiftedKey(int key, Qt::KeyboardModifiers mods, quint32 nativeVirtualKey)
{
    if (!(mods & Qt::ShiftModifier))
        return key;
    if (key >= Qt::Key_A && key <= Qt::Key_Z)
        return key; // letters already arrive unshifted (Key_A for Shift+a)

#ifdef Q_OS_MACOS
    // kVK_ANSI_* virtual keycodes for the non-letter character keys. Only
    // these positions are translated: nativeVirtualKey 0 is kVK_ANSI_A but is
    // also what synthetic/offscreen events carry, so it must not be resolved.
    const int ansiBase = [nativeVirtualKey]() -> int {
    switch (nativeVirtualKey) {
    case 0x12: return Qt::Key_1;
    case 0x13: return Qt::Key_2;
    case 0x14: return Qt::Key_3;
    case 0x15: return Qt::Key_4;
    case 0x17: return Qt::Key_5;
    case 0x16: return Qt::Key_6;
    case 0x1A: return Qt::Key_7;
    case 0x1C: return Qt::Key_8;
    case 0x19: return Qt::Key_9;
    case 0x1D: return Qt::Key_0;
    case 0x1B: return Qt::Key_Minus;
    case 0x18: return Qt::Key_Equal;
    case 0x21: return Qt::Key_BracketLeft;
    case 0x1E: return Qt::Key_BracketRight;
    case 0x2A: return Qt::Key_Backslash;
    case 0x29: return Qt::Key_Semicolon;
    case 0x27: return Qt::Key_Apostrophe;
    case 0x2B: return Qt::Key_Comma;
    case 0x2F: return Qt::Key_Period;
    case 0x2C: return Qt::Key_Slash;
    case 0x32: return Qt::Key_QuoteLeft;
    default: return 0;
    }
    }();
    if (ansiBase) {
        // Prefer what this physical key types unshifted in the CURRENT
        // layout — the same table global-hotkey registration
        // (shortcuts::toCarbon) uses — so the recorded chord binds the key the
        // user actually pressed on non-US layouts. US positions otherwise.
        const int layoutKey = shortcuts::layoutUnshiftedQtKey(nativeVirtualKey);
        return layoutKey ? layoutKey : ansiBase;
    }
#else
    Q_UNUSED(nativeVirtualKey);
#endif

    // Fallback: US-layout shifted symbol -> base key.
    switch (key) {
    case Qt::Key_Exclam:       return Qt::Key_1;
    case Qt::Key_At:           return Qt::Key_2;
    case Qt::Key_NumberSign:   return Qt::Key_3;
    case Qt::Key_Dollar:       return Qt::Key_4;
    case Qt::Key_Percent:      return Qt::Key_5;
    case Qt::Key_AsciiCircum:  return Qt::Key_6;
    case Qt::Key_Ampersand:    return Qt::Key_7;
    case Qt::Key_Asterisk:     return Qt::Key_8;
    case Qt::Key_ParenLeft:    return Qt::Key_9;
    case Qt::Key_ParenRight:   return Qt::Key_0;
    case Qt::Key_Underscore:   return Qt::Key_Minus;
    case Qt::Key_Plus:         return Qt::Key_Equal;
    case Qt::Key_BraceLeft:    return Qt::Key_BracketLeft;
    case Qt::Key_BraceRight:   return Qt::Key_BracketRight;
    case Qt::Key_Bar:          return Qt::Key_Backslash;
    case Qt::Key_Colon:        return Qt::Key_Semicolon;
    case Qt::Key_QuoteDbl:     return Qt::Key_Apostrophe;
    case Qt::Key_Less:         return Qt::Key_Comma;
    case Qt::Key_Greater:      return Qt::Key_Period;
    case Qt::Key_Question:     return Qt::Key_Slash;
    case Qt::Key_AsciiTilde:   return Qt::Key_QuoteLeft;
    default:                   return key;
    }
}

} // namespace hotkeykeymap

#endif // HOTKEYKEYMAP_H
