#pragma once

#include <QJsonObject>
#include <QString>
#include <QStringList>
#include <Qt>
#include <cstdint>

// Single source of truth for the app's global hotkeys.
//
// Three places used to encode the same chords independently — main.cpp
// (Carbon RegisterEventHotKey), TrayIcon (menu labels), and PreferencesWindow
// (the badge editor). They drifted: prefs could "rebind" a chord that the live
// hotkey and the tray label never saw. This namespace centralizes it:
//
//   * chord(actionKey)  -> the current chord string ("Cmd+Shift+S"), read from
//                          the shared config (hotkeys.global.<key>), falling
//                          back to the built-in default.
//   * glyphs(chord)     -> macOS symbol form ("⌘⇧S") for display.
//   * toCarbon(chord..) -> parse into a Carbon virtual keycode + modifier mask
//                          for RegisterEventHotKey (macOS only).
//
// Config is the same JSON that PreferencesWindow saves via snapforge_config_save,
// so a save in prefs + a reload here keeps all three in lockstep.
namespace shortcuts {

// The canonical global hotkeys, in tray-menu order. actionKey matches the JSON
// key under hotkeys.global and the PreferencesWindow row actionKey.
struct GlobalAction {
    const char *actionKey;
    const char *defaultChord;
};
inline constexpr GlobalAction kGlobalActions[] = {
    {"screenshot",  "Cmd+Shift+S"},
    {"fullscreen",  "Cmd+Shift+F"},
    {"record",      "Cmd+Shift+R"},
    {"history",     "Cmd+Shift+H"},
    {"preferences", "Cmd+,"},
};

// Snapshot of the hotkeys section of the shared config, loaded and parsed
// ONCE at construction. Use it for batches of chord lookups (the overlay's
// ~25 rebinds, tray menu rebuilds, Carbon re-registration) — the free chord()
// functions below re-load the config from disk on every call.
class Snapshot {
public:
    Snapshot();

    // Current chord for a global action: config value if present, else the
    // built-in default from kGlobalActions.
    QString chord(const QString &actionKey) const;

    // Current chord for any other Preferences hotkey row
    // (hotkeys.<section>.<actionId>): config value if present, else the
    // caller-supplied default.
    QString chord(const QString &section, const QString &actionId,
                  const QString &defaultChord) const;

private:
    QJsonObject m_hotkeys; // hotkeys.* from config; empty if missing/unparseable
};

// Single-lookup conveniences: load the config once and resolve one chord.
// For several lookups in a row, construct one Snapshot instead.
QString chord(const QString &actionKey);
QString chord(const QString &section, const QString &actionId,
              const QString &defaultChord);

// Parse a chord ("Cmd+Shift+Z", "Escape", "A") into a Qt key + modifier set
// for matching QKeyEvents inside the app. "Cmd" maps to Qt::ControlModifier
// and "Ctrl" to Qt::MetaModifier — Qt swaps Command/Control on macOS, and this
// mirrors PreferencesWindow::formatKeyToShortcut which records the chords.
// Returns false if no key token parses (caller should fall back to a default).
bool toQtKey(const QString &chord, int *key, Qt::KeyboardModifiers *mods);

// Render a chord string ("Cmd+Shift+S") as macOS glyphs ("⌘⇧S").
QString glyphs(const QString &chord);

// Split a chord into its tokens. '+' separates tokens, so the plus KEY is
// written "Plus" ("Cmd+Plus" — what the Preferences recorder emits) or as a
// trailing "++" ("Cmd++", QKeySequence's spelling); both yield the token "+".
QStringList chordTokens(const QString &chord);

// True if `chord` may be registered as a system-wide hotkey: it has at least
// one of Cmd / Ctrl (physical ⌃, Qt::MetaModifier) / Alt — Shift alone or no
// modifier would hijack normal typing — AND its key maps to a virtual keycode
// (toCarbon on macOS). Call as shortcuts::isValidGlobalChord(c) or
// Shortcuts::isValidGlobalChord(c).
bool isValidGlobalChord(const QString &chord);

#ifdef Q_OS_MACOS
// Parse a chord into a Carbon virtual keycode (*vk) and modifier mask (*mods,
// cmdKey/shiftKey/optionKey/controlKey). Returns false if the key token has no
// known virtual keycode (caller should skip registration and warn). Character
// keys are resolved through the current keyboard layout (UCKeyTranslate), so
// "Cmd+Z" binds the key labelled Z on QWERTZ/AZERTY too; falls back to the US
// ANSI positions. Supports letters, digits, punctuation, Plus, Space, Return,
// Tab, Escape, Backspace (⌫), Delete (⌦), arrows, Home/End/PageUp/PageDown
// and F1–F12. Does NOT check modifiers — use isValidGlobalChord for that.
bool toCarbon(const QString &chord, uint32_t *vk, uint32_t *mods);

// Qt::Key for what the main-block key `nativeVirtualKey` types with NO
// modifiers in the current keyboard layout — the inverse of the lookup
// toCarbon() registers through. Returns 0 if unknown, or when
// SNAPFORGE_HOTKEY_ANSI_ONLY=1 (tests), so callers fall back to US positions.
int layoutUnshiftedQtKey(quint32 nativeVirtualKey);
#endif

} // namespace shortcuts

// CamelCase alias so UI code can spell it Shortcuts::isValidGlobalChord(...).
namespace Shortcuts = shortcuts;
