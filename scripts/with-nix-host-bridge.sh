#!/usr/bin/env bash
# Bridges this project's Nix devShell to host-only pieces its Tauri/WebKit/
# CUDA runtime needs that Nix either can't provide (the proprietary NVIDIA
# driver) or, per a deliberate decision, doesn't (webkit2gtk: WebKit's own
# helper-process binaries hardcode their nix store path internally, so a
# Nix-patched WebKit would need a full from-source rebuild just to relink
# two `patchelf` calls -- not worth it; `sudo pacman -S webkit2gtk-4.1` on
# the host instead). Nix's own dynamic linker doesn't consult the host's
# ld.so.cache or its Mesa/GBM driver directories by default, so none of
# this is optional once any of those pieces are in play.
set -euo pipefail

# Nix's gdk-pixbuf doesn't register png/jpeg (built-in, but not listed in
# its own loaders.cache) *or* svg (a separate package, librsvg, whose own
# setup-hook points GDK_PIXBUF_MODULE_FILE at an svg-only cache that
# shadows gdk-pixbuf's) in a way GTK's module-based format sniffing can
# see -- combine both into one cache gdk-pixbuf-query-loaders can discover.
# Must run before the PKG_CONFIG_PATH edit below: the host has its own
# gdk-pixbuf-2.0.pc/librsvg-2.0.pc too, and PKG_CONFIG_PATH search order
# would otherwise resolve *its* (wrong, empty-glob) libdir instead of
# Nix's.
cache_dir="${XDG_CACHE_HOME:-$HOME/.cache}/holzi"
mkdir -p "$cache_dir"
cache="$cache_dir/gdk-pixbuf-loaders.cache"
gdk_pixbuf_libdir="$(pkg-config --variable=libdir gdk-pixbuf-2.0)"
librsvg_libdir="$(pkg-config --variable=libdir librsvg-2.0)"
gdk-pixbuf-query-loaders \
  "$gdk_pixbuf_libdir"/gdk-pixbuf-2.0/2.10.0/loaders/*.so \
  "$librsvg_libdir"/gdk-pixbuf-2.0/2.10.0/loaders/*.so \
  >"$cache" 2>/dev/null || true
export GDK_PIXBUF_MODULE_FILE="$cache"

# webkit2gtk-4.1 is deliberately host-provided, not Nix (WebKit's own
# helper-process binaries hardcode their nix store path internally, so a
# Nix-patched WebKit would need a full from-source rebuild just to relink
# two `patchelf` calls -- not worth it; `sudo pacman -S webkit2gtk-4.1` on
# the host instead). Nix's own PKG_CONFIG_PATH doesn't include the host's,
# by design, so the build can't find it without this.
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
# Deliberately not exporting LD_LIBRARY_PATH=...:/usr/lib here: that would
# leak into every child process this devShell spawns, not just holzi's own
# binary, and any *other* Nix-provided binary that happens to touch
# /usr/lib/libc.so.6 (observed: plain `mkdir`) hits the same glibc clash
# src-tauri/build.rs works around for holzi's own binary via `-rpath`
# (process-scoped, not environment-wide).

# Mesa's GBM/DRI loader defaults to the NixOS-only `/run/opengl-driver`
# convention; GLVND's EGL vendor dispatch defaults similarly. Point both at
# where this host (Arch/CachyOS-family; adjust for other distros) actually
# keeps them.
export GBM_BACKENDS_PATH=/usr/lib/gbm
export __EGL_VENDOR_LIBRARY_DIRS=/usr/share/glvnd/egl_vendor.d

# GTK's own client-side-decoration title bar (minimize/maximize/close
# buttons) renders symbolic icons via GResource-embedded PNGs, loaded
# through GdkPixbufLoader's streaming API rather than gdk_pixbuf_new_from_
# file -- which, unlike the file path, doesn't recognize png as a
# supported format in this Nix gdk-pixbuf build even with a correct
# loaders.cache (verified: real PNG bytes, confirmed via `gresource
# extract`; file-based decoding of the same bytes on disk works fine
# standalone). GTK_CSD=0 would normally avoid needing this by falling
# back to window-manager-drawn decorations instead, but has no effect
# under Wayland (CSD is unconditional there without the compositor
# implementing xdg-decoration) -- hence also forcing the X11 backend
# (XWayland), where GTK_CSD=0 is honored.
export GDK_BACKEND=x11
export GTK_CSD=0

exec "$@"
