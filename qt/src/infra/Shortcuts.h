#pragma once

#include <QString>
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
    {"screenshot", "Cmd+Shift+S"},
    {"fullscreen", "Cmd+Shift+F"},
    {"record",     "Cmd+Shift+R"},
    {"history",    "Cmd+Shift+H"},
};

// Current chord for a global action: config value if present, else the default.
QString chord(const QString &actionKey);

// Render a chord string ("Cmd+Shift+S") as macOS glyphs ("⌘⇧S").
QString glyphs(const QString &chord);

#ifdef Q_OS_MAC
// Parse a chord into a Carbon virtual keycode (*vk) and modifier mask (*mods,
// cmdKey/shiftKey/optionKey/controlKey). Returns false if the key token has no
// known virtual keycode (caller should skip registration and warn).
bool toCarbon(const QString &chord, uint32_t *vk, uint32_t *mods);
#endif

} // namespace shortcuts
