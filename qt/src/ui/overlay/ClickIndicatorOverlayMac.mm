#include <QWindow>

#import <AppKit/AppKit.h>

// Sit above normal windows and appear on every Space. Without these the
// overlay is hidden by full-screen apps and only paints on the current
// Space. Also marks the window as non-interactive so it never intercepts
// the user's clicks.
void clickoverlay_configure_macos_window(QWindow *w)
{
    if (!w) return;
    NSView *nsview = reinterpret_cast<NSView *>(w->winId());
    if (!nsview) return;
    NSWindow *nswindow = [nsview window];
    if (!nswindow) return;

    [nswindow setLevel:NSScreenSaverWindowLevel];
    [nswindow setCollectionBehavior:
        NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehaviorStationary
        | NSWindowCollectionBehaviorFullScreenAuxiliary
        | NSWindowCollectionBehaviorIgnoresCycle];
    [nswindow setIgnoresMouseEvents:YES];
}
