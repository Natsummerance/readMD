#!/usr/bin/env bash
# ReadMD Linux & 信创全架构 (x86_64 / aarch64 / loongarch64) 与鸿蒙打包脚本
set -euo pipefail

VERSION="${READMD_BUILD_VERSION:-${READMD_VERSION:-$(python3 -c 'import readmd; print(readmd.VERSION)' 2>/dev/null || echo '2.3.2')}}"
ARCH="$(uname -m)"
DEB_ARCH="amd64"
if [ "${ARCH}" = "aarch64" ]; then
  DEB_ARCH="arm64"
elif [ "${ARCH}" = "loongarch64" ]; then
  DEB_ARCH="loongarch64"
fi

echo "=== Building ReadMD v${VERSION} for Linux (${ARCH} / ${DEB_ARCH}) ==="

# 1. PyInstaller 构建独立二进制
python3 -m PyInstaller \
  --noconfirm \
  --clean \
  --onedir \
  --name "ReadMD" \
  --icon "assets/ReadMD.png" \
  --add-data "assets:assets" \
  --add-data "src/readmd_modules:src/readmd_modules" \
  --add-data "src/readmd_fix.py:src" \
  --hidden-import src.readmd_fix \
  --collect-data magika \
  --collect-data docx \
  --collect-data reportlab \
  --collect-data matplotlib \
  --collect-data trafilatura \
  --collect-submodules src.readmd_modules \
  readmd.py

# 2. 构建 AppDir
APPDIR="dist/ReadMD.AppDir"
rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin" "${APPDIR}/usr/share/applications" "${APPDIR}/usr/share/icons/hicolor/512x512/apps" "${APPDIR}/usr/share/mime/packages"

cp -r dist/ReadMD/* "${APPDIR}/usr/bin/"
cp scripts/linux/io.github.natsummerance.readmd.desktop "${APPDIR}/usr/share/applications/"
cp scripts/linux/io.github.natsummerance.readmd.desktop "${APPDIR}/"
cp assets/ReadMD.png "${APPDIR}/usr/share/icons/hicolor/512x512/apps/readmd.png"
cp assets/ReadMD.png "${APPDIR}/readmd.png"
cp scripts/linux/readmd.xml "${APPDIR}/usr/share/mime/packages/"

# 创建 AppRun 启动入口
cat << 'EOF' > "${APPDIR}/AppRun"
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH:-}"
export WEBKIT_DISABLE_COMPOSITING_MODE=0
exec "${HERE}/usr/bin/ReadMD" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

# 3. 构建 AppImage
APPIMAGE_NAME="ReadMD-linux-${ARCH}-v${VERSION}.AppImage"
if command -v appimagetool >/dev/null 2>&1; then
  appimagetool "${APPDIR}" "dist/${APPIMAGE_NAME}"
elif [ -f "./appimagetool" ]; then
  ./appimagetool --appimage-extract-and-run "${APPDIR}" "dist/${APPIMAGE_NAME}"
else
  echo "Downloading appimagetool to create AppImage..."
  wget -q https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage -O ./appimagetool || true
  if [ -f "./appimagetool" ]; then
    chmod +x ./appimagetool
    ./appimagetool --appimage-extract-and-run "${APPDIR}" "dist/${APPIMAGE_NAME}" || true
  fi
fi

# 4. 构建 DEB 安装包 (适用于 Ubuntu / Debian / 统信 UOS / 银河麒麟 KylinOS / 深度 Deepin)
DEB_DIR="dist/deb_build"
rm -rf "${DEB_DIR}"
mkdir -p "${DEB_DIR}/DEBIAN" "${DEB_DIR}/usr/bin" "${DEB_DIR}/usr/share/applications" "${DEB_DIR}/usr/share/icons/hicolor/512x512/apps" "${DEB_DIR}/usr/share/mime/packages" "${DEB_DIR}/opt/readmd"

cp -r dist/ReadMD/* "${DEB_DIR}/opt/readmd/"
ln -sf /opt/readmd/ReadMD "${DEB_DIR}/usr/bin/readmd"
cp scripts/linux/io.github.natsummerance.readmd.desktop "${DEB_DIR}/usr/share/applications/"
cp assets/ReadMD.png "${DEB_DIR}/usr/share/icons/hicolor/512x512/apps/readmd.png"
cp scripts/linux/readmd.xml "${DEB_DIR}/usr/share/mime/packages/"

cat << EOF > "${DEB_DIR}/DEBIAN/control"
Package: readmd
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: Natsummerance <natsummerance@github.com>
Description: ReadMD - Lightweight Markdown viewer and editor with auto-repair, LaTeX PRO, and multi-platform support.
EOF

dpkg-deb --build "${DEB_DIR}" "dist/readmd_${VERSION}_${DEB_ARCH}.deb" || echo "dpkg-deb not available, skipping deb package"

# 5. 构建 HarmonyOS NEXT 原生 HAP 包 (遵循 OpenHarmony 规范归档)
if [ -d "packages/harmonyos-app" ]; then
  mkdir -p dist
  (cd packages/harmonyos-app && zip -r -q "../../dist/ReadMD-harmonyos-v${VERSION}.hap" .)
fi

echo "=== Linux and multi-platform packaging completed ==="
