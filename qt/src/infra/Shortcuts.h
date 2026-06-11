#pragma once

#include <QString>
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

// Current chord for a global action: config value if present, else the default.
QString chord(const QString &actionKey);

// Current chord for any other Preferences hotkey row (hotkeys.<section>.<actionId>):
// config value if present, else the caller-supplied default. The overlay uses
// this for its local tools/sizes/actions bindings.
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

#ifdef Q_OS_MAC
// Parse a chord into a Carbon virtual keycode (*vk) and modifier mask (*mods,
// cmdKey/shiftKey/optionKey/controlKey). Returns false if the key token has no
// known virtual keycode (caller should skip registration and warn).
bool toCarbon(const QString &chord, uint32_t *vk, uint32_t *mods);
#endif

} // namespace shortcuts
