#!/usr/bin/env bash
# ReadMD Linux 原生打包脚本（本版本正式支持 x86_64 / aarch64）。
# 其他架构必须使用专门的、已验证的构建流程，不得被此脚本误标为正式产物。
set -euo pipefail
if [ -z "${READMD_VERSION:-}" ]; then
  if [ -f ".env" ]; then
    READMD_VERSION="$(grep -E '^READMD_VERSION=' .env | head -n1 | cut -d'=' -f2- | tr -d '\r\"\'')"
  elif [ -f "VERSION" ]; then
    READMD_VERSION="$(head -n1 VERSION | tr -d '\r\n')"
  else
    READMD_VERSION="$(python3 -c 'from src.readmd_core.config import get_version; print(get_version())' 2>/dev/null)"
  fi
fi
VERSION="${READMD_BUILD_VERSION:-${READMD_VERSION}}"
ARCH="$(uname -m)"
DEB_ARCH="${READMD_DEB_ARCH:-}"
if [ -z "${DEB_ARCH}" ]; then
  case "${ARCH}" in
    aarch64|arm64) DEB_ARCH="arm64" ;;
    x86_64|amd64)   DEB_ARCH="amd64" ;;
    *)
      echo "ERROR: unsupported architecture for the v2.3.7 Linux release: ${ARCH}" >&2
      exit 2
      ;;
  esac
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
  --add-data "src/readmd_core:src/readmd_core" \
  --add-data "src/readmd_modules:src/readmd_modules" \
  --add-data "src/readmd_fix.py:src" \
  --hidden-import src.readmd_fix \
  --hidden-import src.readmd_core \
  --collect-data magika \
  --collect-data docx \
  --collect-data reportlab \
  --collect-data matplotlib \
  --collect-data trafilatura \
  --collect-submodules src.readmd_core \
  --collect-submodules src.readmd_modules \
  readmd.py

# 2. 校验构建出的 ELF 二进制架构与目标包架构一致性
if [ -f "dist/ReadMD/ReadMD" ] && command -v file >/dev/null 2>&1; then
  BIN_INFO="$(file dist/ReadMD/ReadMD)"
  echo "Verifying binary architecture: ${BIN_INFO}"
  case "${DEB_ARCH}" in
    arm64)
      if ! echo "${BIN_INFO}" | grep -Eqi 'aarch64|ARM aarch64'; then
        echo "ERROR: Target deb architecture is arm64, but binary is not ARM64: ${BIN_INFO}" >&2
        exit 1
      fi
      ;;
    amd64)
      if ! echo "${BIN_INFO}" | grep -Eqi 'x86-64|x86_64'; then
        echo "ERROR: Target deb architecture is amd64, but binary is not x86_64: ${BIN_INFO}" >&2
        exit 1
      fi
      ;;
  esac
fi

# 3. 构建 AppDir
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
if [ "$(uname -m)" = "aarch64" ] && { grep -Eqi 'phytium|ft-[0-9]{3,4}|feiteng|tengyun|d2000|e2000|s2500' /proc/cpuinfo 2>/dev/null || grep -Eqi 'phytium|feiteng|d2000|e2000|s2500' /proc/device-tree/model /proc/device-tree/vendor 2>/dev/null; }; then
  # Phytium/Kylin boards ship unstable vendor GL stacks; UKUI + X11 + llvmpipe
  # is the tested-safe path for WebKitGTK.
  export GDK_BACKEND="${GDK_BACKEND:-x11}"
  export WEBKIT_DISABLE_COMPOSITING_MODE=1
  export WEBKIT_DISABLE_DMABUF_RENDERER=1
  export LIBGL_ALWAYS_SOFTWARE=1
  export GALLIUM_DRIVER=llvmpipe
else
  export WEBKIT_DISABLE_COMPOSITING_MODE=0
fi
exec "${HERE}/usr/bin/ReadMD" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

# 4. 构建 AppImage
APPIMAGE_NAME="ReadMD-linux-${ARCH}-v${VERSION}.AppImage"
APPIMAGE_BUILT=0
if command -v appimagetool >/dev/null 2>&1; then
  appimagetool "${APPDIR}" "dist/${APPIMAGE_NAME}"
  APPIMAGE_BUILT=1
elif [ -f "./appimagetool" ]; then
  ./appimagetool --appimage-extract-and-run "${APPDIR}" "dist/${APPIMAGE_NAME}"
  APPIMAGE_BUILT=1
else
  case "${ARCH}" in
    aarch64) APPIMAGE_TOOL_ARCH="aarch64" ;;
    *) APPIMAGE_TOOL_ARCH="x86_64" ;;
  esac
  APPIMAGE_TOOL="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/readmd-appimagetool-${APPIMAGE_TOOL_ARCH}"
  echo "Downloading ${APPIMAGE_TOOL_ARCH} appimagetool..."
  if wget -q "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${APPIMAGE_TOOL_ARCH}.AppImage" -O "${APPIMAGE_TOOL}"; then
    chmod +x "${APPIMAGE_TOOL}"
    "${APPIMAGE_TOOL}" --appimage-extract-and-run "${APPDIR}" "dist/${APPIMAGE_NAME}"
    APPIMAGE_BUILT=1
  fi
  rm -f "${APPIMAGE_TOOL}"
fi
if [ "${APPIMAGE_BUILT}" -ne 1 ] || [ ! -s "dist/${APPIMAGE_NAME}" ]; then
  echo "ERROR: AppImage creation failed; refusing to report a green Linux build." >&2
  exit 1
fi

# 5. 构建 DEB 安装包 (适用于 Ubuntu / Debian / 统信 UOS / 银河麒麟 KylinOS / 深度 Deepin / openEuler)
DEB_DIR="dist/deb_build"
rm -rf "${DEB_DIR}"
mkdir -p "${DEB_DIR}/DEBIAN" "${DEB_DIR}/usr/bin" "${DEB_DIR}/usr/share/applications" "${DEB_DIR}/usr/share/icons/hicolor/512x512/apps" "${DEB_DIR}/usr/share/mime/packages" "${DEB_DIR}/opt/readmd"

cp -r dist/ReadMD/* "${DEB_DIR}/opt/readmd/"
ln -sf /opt/readmd/ReadMD "${DEB_DIR}/usr/bin/readmd"
cp scripts/linux/io.github.natsummerance.readmd.desktop "${DEB_DIR}/usr/share/applications/"
cp assets/ReadMD.png "${DEB_DIR}/usr/share/icons/hicolor/512x512/apps/readmd.png"
cp scripts/linux/readmd.xml "${DEB_DIR}/usr/share/mime/packages/"

# 控制文件：原生窗口和 OCR 是正式功能的硬依赖，不能降级为浏览器或缺失组件。
cat << EOF > "${DEB_DIR}/DEBIAN/control"
Package: readmd
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: Natsummerance <natsummerance@github.com>
Homepage: https://readmd.asia
Description: ReadMD - Lightweight Markdown viewer and editor with auto-repair, LaTeX PRO, and multi-platform support.
Depends: libc6, libgtk-3-0, libwebkit2gtk-4.0-37 | libwebkit2gtk-4.1-0 | libwebkit2gtk-6.0-4, xdg-utils, shared-mime-info, tesseract-ocr
Recommends: gir1.2-webkit2-4.0 | gir1.2-webkit2-4.1 | gir1.2-webkit-6.0, gir1.2-gtk-3.0, libnotify-bin
Suggests: kylin-browser | uos-browser | chromium-browser | google-chrome-stable | firefox
EOF

cat << 'EOF' > "${DEB_DIR}/DEBIAN/postinst"
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
  fi
  if command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database /usr/share/mime || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
  fi
  chmod +x /opt/readmd/ReadMD 2>/dev/null || true
  ln -sf /opt/readmd/ReadMD /usr/bin/readmd
fi
exit 0
EOF
chmod 755 "${DEB_DIR}/DEBIAN/postinst"

cat << 'EOF' > "${DEB_DIR}/DEBIAN/postrm"
#!/bin/sh
set -e
if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
  rm -f /usr/bin/readmd
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
  fi
  if command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database /usr/share/mime || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
  fi
fi
exit 0
EOF
chmod 755 "${DEB_DIR}/DEBIAN/postrm"

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "ERROR: dpkg-deb is required to produce the Linux release asset." >&2
  exit 1
fi
DEB_OUTPUT="dist/readmd_${VERSION}_${DEB_ARCH}.deb"
dpkg-deb --build "${DEB_DIR}" "${DEB_OUTPUT}"
test -s "${DEB_OUTPUT}"

echo "=== Linux and multi-platform packaging completed ==="
