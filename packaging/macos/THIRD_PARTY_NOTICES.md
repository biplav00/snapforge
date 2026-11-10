# Third-party notices — Snapforge for macOS

Snapforge is distributed with the third-party components listed below. Their
licence texts are in this folder (`Snapforge.app/Contents/Resources/Licenses/`).

## FFmpeg (GPL)

- **What ships:** the `ffmpeg` command-line program (`Contents/MacOS/ffmpeg`)
  and the FFmpeg and codec libraries it needs (`Contents/Frameworks/*.dylib`),
  taken from the Homebrew `ffmpeg` package.
- **Licence:** this FFmpeg build is configured with `--enable-gpl` and
  `--enable-version3`, so it is distributed under the **GNU General Public
  License, version 3 or later**. The exact licence FFmpeg reports for itself is
  in `ffmpeg/LICENSE-ffmpeg-L.txt`. The licence texts are in `ffmpeg/`
  (`COPYING.GPLv3`, `COPYING.GPLv2`, `COPYING.LGPLv3`, `COPYING.LGPLv2.1`).
- **How Snapforge uses it:** Snapforge starts `ffmpeg` as a **separate
  process** and sends it raw video frames through a pipe. Snapforge does not
  link against FFmpeg's libraries.
- **Exact version:** the version and build configuration of the bundled
  binary are in `ffmpeg/VERSION.txt`.

### Codec libraries (x264, x265, and others)

FFmpeg's bundled dependencies include libx264 (GPL-2.0-or-later) and libx265
(GPL-2.0-or-later), plus other codec, container and crypto libraries. Each
library's own licence files are in a folder named after its Homebrew formula
(for example `x264/COPYING` and `x265/COPYING`).

## Qt (LGPLv3)

- **What ships:** Qt 6 frameworks and plugins (`Contents/Frameworks/Qt*.framework`,
  `Contents/PlugIns/`), linked dynamically.
- **Licence:** GNU Lesser General Public License v3 (`qt/COPYING.LGPLv3`,
  which builds on `qt/COPYING.GPLv3`). The Qt version is recorded in
  `qt/README.txt`.
- You may replace the bundled Qt frameworks with your own build of the same
  Qt version. Note that changing files in the bundle breaks its code
  signature, so you will need to re-sign it (an ad-hoc `codesign --force
  --deep --sign -` is enough).

## Source code

### FFmpeg and the bundled GPL/LGPL libraries: shipped with every release

Each Snapforge release on GitHub has a
`Snapforge-<version>-third-party-sources.tar.gz` file next to the `.dmg`. It
contains the complete corresponding source for FFmpeg and every bundled
library whose licence is GPL or LGPL (including x264, x265, LAME, mpg123,
gettext/libintl, GLib, xz, zstd, graphite2 and D-Bus):

- the exact upstream source archive each bundled binary was built from,
- Homebrew's build recipe (`<formula>.rb`: configure flags, dependencies,
  inline patches) and any external patches it applies, and
- `MANIFEST.tsv`, listing every bundled Homebrew library with its version,
  licence and the SHA-256 of the included source archive.

Get it from the same release page as the `.dmg`
(<https://github.com/biplav00/snapforge/releases>).

### Qt: written offer

Qt's source (about 1 GB) is not in that archive. The exact Qt version is in
`qt/README.txt` (and in `qt/SOURCE.txt` in the sources archive), and its source
is available upstream at <https://download.qt.io/archive/qt/> — use
`<major.minor>/<version>/single/qt-everywhere-src-<version>.tar.xz`. Homebrew's
Qt build recipe is included in the sources archive as `qt/qtbase.rb`.

In addition, for **three years** from the date we distribute a given
Snapforge release, we will give anyone who asks a complete machine-readable
copy of the source code for Qt — or for any other GPL/LGPL component shipped
in that release, should the sources archive ever be unavailable — at no more
than our cost of physically distributing it. Ask by opening an issue on the
Snapforge GitHub repository and including the Snapforge version and the
contents of `ffmpeg/VERSION.txt` and `qt/README.txt`.

Upstream project pages, for reference:

- FFmpeg: <https://ffmpeg.org/releases/> (version in `ffmpeg/VERSION.txt`)
- x264: <https://code.videolan.org/videolan/x264>
- x265: <https://bitbucket.org/multicoreware/x265_git>
- Other libraries: `https://formulae.brew.sh/formula/<name>`

Snapforge's own source code is available from its GitHub repository.
