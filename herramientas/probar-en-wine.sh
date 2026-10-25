#!/bin/bash
# Probar CAD-pixpin en Linux con Wine (sin Windows a mano): compila con MinGW,
# arranca una pantalla virtual y la app, y deja el DISPLAY :99 para mandarle
# clics y teclas con xdotool, sacar capturas (import) o grabar (ffmpeg).
#
# Una vez: apt-get install wine64 winetricks cabextract xvfb xdotool \
#   imagemagick ffmpeg gcc-mingw-w64-x86-64 && rustup target add x86_64-pc-windows-gnu
# y el compilador de sombreadores de Microsoft (el de Wine no entiende el del
# visor): WINE=/usr/lib/wine/wine64 WINEPREFIX=~/.wine-cad winetricks -q d3dcompiler_47
#
# Uso: herramientas/probar-en-wine.sh [plano.dwg]
set -e
cd "$(dirname "$0")/.."
cargo build -p cadpixpin --target x86_64-pc-windows-gnu
export DISPLAY=:99 WINEDEBUG=-all WINEPREFIX="${WINEPREFIX:-$HOME/.wine-cad}" \
  WINEDLLOVERRIDES="d3dcompiler_47=n"
pgrep -x Xvfb >/dev/null || (Xvfb :99 -screen 0 1600x900x24 >/dev/null 2>&1 &)
sleep 1
wineserver -k 2>/dev/null || true
nohup wine target/x86_64-pc-windows-gnu/debug/cadpixpin.exe "$@" >/tmp/cad-app.log 2>&1 &
echo "CAD-pixpin arrancando en DISPLAY=:99 (registro en /tmp/cad-app.log)"
