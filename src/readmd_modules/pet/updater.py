# -*- coding: utf-8 -*-
"""ReadMD desktop pet auto-update engine.

Supports:
1. Syncing with software updates / local candidate archives (ReadMD-Desktop-Pet.zip).
2. Online checking and downloading from GitHub Releases (with proxy acceleration).
3. Seamless hot-swap and restart of the desktop pet runtime.
4. Auto-migration away from legacy %APPDATA% (C: drive) locations.
"""

from __future__ import annotations

import json
import logging
import os
import shutil
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

from src.readmd_core.versioning import compare_versions, parse_version
from .hermes_adapter import HermesPetLauncher, HermesPetPluginInstaller, get_app_install_dir

GITHUB_REPO = 'Natsummerance/readMD'
GITHUB_API_LATEST = f'https://api.github.com/repos/{GITHUB_REPO}/releases/latest'
GITHUB_API_RELEASES = f'https://api.github.com/repos/{GITHUB_REPO}/releases?per_page=100'

MIRROR_PREFIXES = [
    'https://ghfast.top/',
    'https://ghproxy.net/',
    'https://mirror.ghproxy.com/',
]


def clean_legacy_pet_installations(active_target: Path) -> List[str]:
    """Safely remove legacy installations under %APPDATA% (C: drive) if active target is elsewhere."""
    cleaned = []
    appdata = os.environ.get('APPDATA')
    if not appdata:
        return cleaned
    legacy_candidates = [
        Path(appdata) / 'ReadMD' / 'plugins' / 'pet' / 'hermes-adapter',
        Path(appdata) / 'ReadMD' / 'pet' / 'hermes-adapter',
    ]
    try:
        resolved_active = active_target.resolve()
    except (OSError, ValueError):
        resolved_active = active_target
    for legacy in legacy_candidates:
        try:
            if legacy.is_dir() and legacy.resolve() != resolved_active:
                shutil.rmtree(legacy, ignore_errors=True)
                if not legacy.exists():
                    cleaned.append(str(legacy))
                    logging.info('Cleaned legacy C: drive pet installation at %s', legacy)
        except OSError:
            pass

    # Clean up empty parent directories or orphan state files under APPDATA if active_target is not in them
    for parent_cand in [Path(appdata) / 'ReadMD' / 'plugins' / 'pet', Path(appdata) / 'ReadMD' / 'pet']:
        try:
            if parent_cand.is_dir() and parent_cand.resolve() != resolved_active:
                try:
                    if resolved_active.is_relative_to(parent_cand.resolve()):
                        continue
                except (AttributeError, ValueError):
                    if str(resolved_active).lower().startswith(str(parent_cand.resolve()).lower()):
                        continue
                has_files = False
                for item in parent_cand.iterdir():
                    if item.is_dir():
                        has_files = True
                        break
                    if item.is_file() and item.name not in ('hermes-overlay-state.json', 'hermes-overlay-state.json.command'):
                        has_files = True
                        break
                if not has_files:
                    shutil.rmtree(parent_cand, ignore_errors=True)
        except OSError:
            pass
    return cleaned


def find_bundled_candidate_archives(app_install_dir: Optional[Path] = None) -> List[Path]:
    """Scan application install and package directories for local candidate archives."""
    install_dir = app_install_dir or get_app_install_dir()
    roots = [
        install_dir,
        install_dir / 'dist',
        install_dir / 'dist' / 'ReadMD',
        install_dir / 'packages' / 'readmd-hermes-pet-adapter' / 'dist',
    ]
    candidate_names = (
        'ReadMD-Desktop-Pet.zip',
        'ReadMD-Desktop-Pet-review.zip',
        'readmd-hermes-pet-adapter-v0.1.0.zip',
    )
    found: List[Path] = []
    seen = set()
    for root in roots:
        if not root.is_dir():
            continue
        try:
            root_res = root.resolve()
            if root_res in seen:
                continue
            seen.add(root_res)
        except OSError:
            continue
        for name in candidate_names:
            archive = root / name
            if archive.is_file():
                try:
                    res_arch = archive.resolve()
                    if res_arch not in seen:
                        seen.add(res_arch)
                        found.append(archive)
                except OSError:
                    pass
    return found


def _get_archive_manifest_hash(archive_path: Path) -> Optional[str]:
    import hashlib
    try:
        with zipfile.ZipFile(archive_path) as zf:
            if 'readmd-pet-plugin.json' in zf.namelist():
                data = zf.read('readmd-pet-plugin.json')
                return hashlib.sha256(data).hexdigest()
    except (OSError, zipfile.BadZipFile):
        pass
    return None


def _fetch_github_json(url: str, timeout: float = 12.0) -> Any:
    urls_to_try = [url] + [prefix + url for prefix in MIRROR_PREFIXES]
    headers = {
        'Accept': 'application/vnd.github.v3+json',
        'User-Agent': 'ReadMD-Desktop-Pet-Updater',
    }
    last_err = None
    for attempt_url in urls_to_try:
        try:
            req = urllib.request.Request(attempt_url, headers=headers)
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                if resp.status == 200:
                    raw = resp.read(2 * 1024 * 1024)
                    return json.loads(raw.decode('utf-8'))
        except Exception as exc:
            last_err = exc
            continue
    if last_err:
        raise last_err
    raise RuntimeError('No mirror succeeded')


def check_pet_update(
    installer: HermesPetPluginInstaller,
    launcher: HermesPetLauncher,
    candidate_archives: Optional[List[Path]] = None,
    allow_network: bool = True,
) -> Dict[str, Any]:
    """Check whether a desktop pet update is available (bundled or GitHub)."""
    is_installed = launcher.status().get('available', False)
    installed_manifest_hash = installer.get_installed_manifest_hash()
    release_info = installer.get_release_info()

    # Step 1: Check bundled candidate archive (Follow Software Update)
    candidates = candidate_archives if candidate_archives is not None else find_bundled_candidate_archives()
    for cand in candidates:
        cand_hash = _get_archive_manifest_hash(cand)
        if cand_hash and cand_hash != installed_manifest_hash:
            return {
                'ok': True,
                'has_update': True,
                'source': 'bundled',
                'version': 'bundled_update',
                'archive_path': str(cand.resolve()),
                'installed': is_installed,
                'install_path': str(installer.target),
                'reason': '检测到软件内置了更新版本的伴侣桌宠包',
            }

    # Step 2: Check GitHub Releases
    if allow_network:
        try:
            rel = _fetch_github_json(GITHUB_API_LATEST)
            if isinstance(rel, dict):
                assets = rel.get('assets', [])
                tag_name = str(rel.get('tag_name') or '').strip()
                pet_asset = None
                for a in assets:
                    name = str(a.get('name') or '').lower()
                    if name in ('readmd-desktop-pet.zip', 'readmd-hermes-pet-adapter-v0.1.0.zip') or ('pet' in name and name.endswith('.zip')):
                        pet_asset = a
                        break
                if pet_asset:
                    asset_url = pet_asset.get('browser_download_url')
                    asset_size = pet_asset.get('size', 0)
                    asset_updated = pet_asset.get('updated_at', '')
                    
                    is_newer = False
                    if not is_installed:
                        is_newer = True
                    else:
                        installed_tag = str(release_info.get('release_tag') or '')
                        if installed_tag and tag_name:
                            is_newer = compare_versions(tag_name, installed_tag) == 1
                        elif asset_updated and asset_updated != release_info.get('asset_updated_at'):
                            is_newer = True
                    
                    if is_newer:
                        return {
                            'ok': True,
                            'has_update': True,
                            'source': 'github',
                            'version': tag_name or 'latest',
                            'asset_name': pet_asset.get('name'),
                            'download_url': asset_url,
                            'size': asset_size,
                            'release_name': rel.get('name', ''),
                            'release_notes': str(rel.get('body') or '')[:600],
                            'installed': is_installed,
                            'install_path': str(installer.target),
                            'reason': f'GitHub 发布了最新桌宠版本 {tag_name}',
                        }
        except Exception as exc:
            logging.debug('GitHub pet update check failed or offline: %s', exc)

    return {
        'ok': True,
        'has_update': False,
        'source': 'none',
        'current_version': release_info.get('release_tag') or 'bundled',
        'installed': is_installed,
        'install_path': str(installer.target),
    }


def download_github_asset(url: str, dest_path: Path, progress_callback: Optional[Callable[[int, int], None]] = None) -> bool:
    """Download an asset from GitHub with mirror fallback and streaming."""
    urls_to_try = [url] + [prefix + url for prefix in MIRROR_PREFIXES]
    headers = {
        'User-Agent': 'ReadMD-Desktop-Pet-Updater',
        'Accept': 'application/octet-stream',
    }
    for attempt_url in urls_to_try:
        try:
            req = urllib.request.Request(attempt_url, headers=headers)
            with urllib.request.urlopen(req, timeout=30.0) as resp:
                if resp.status != 200:
                    continue
                total = int(resp.headers.get('Content-Length', 0) or 0)
                downloaded = 0
                chunk_size = 1024 * 128
                with dest_path.open('wb') as out_f:
                    while True:
                        chunk = resp.read(chunk_size)
                        if not chunk:
                            break
                        out_f.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback and total > 0:
                            progress_callback(downloaded, total)
                if dest_path.is_file() and dest_path.stat().st_size > 1000:
                    return True
        except Exception as exc:
            logging.debug('Download attempt failed on %s: %s', attempt_url, exc)
            if dest_path.exists():
                try:
                    dest_path.unlink()
                except OSError:
                    pass
            continue
    return False


def apply_pet_update(
    installer: HermesPetPluginInstaller,
    launcher: HermesPetLauncher,
    update_info: Dict[str, Any],
    progress_callback: Optional[Callable[[int, int], None]] = None,
) -> Dict[str, Any]:
    """Apply a checked pet update cleanly with rollback safety and smooth process reload."""
    source_kind = update_info.get('source')
    was_running = launcher.status().get('running', False)
    if was_running:
        launcher.stop()

    try:
        if source_kind == 'bundled':
            archive_path = update_info.get('archive_path')
            if not archive_path or not os.path.isfile(archive_path):
                return {'ok': False, 'code': 'bundled_archive_not_found'}
            res = installer.install_archive(archive_path, confirm=True)
            if res.get('ok'):
                installer.set_release_info({
                    'source': 'bundled',
                    'updated_at': time.time(),
                    'installed_archive': os.path.basename(archive_path),
                })
                clean_legacy_pet_installations(installer.target)
                if was_running:
                    launcher.start()
                return {'ok': True, 'updated': True, 'install_path': str(installer.target), 'source': 'bundled'}
            return res

        elif source_kind == 'github':
            download_url = update_info.get('download_url')
            if not download_url:
                return {'ok': False, 'code': 'missing_download_url'}
            with tempfile.TemporaryDirectory(prefix='readmd-pet-dl-') as tmpdir:
                dl_path = Path(tmpdir) / 'ReadMD-Desktop-Pet.zip'
                ok = download_github_asset(download_url, dl_path, progress_callback)
                if not ok:
                    return {'ok': False, 'code': 'pet_download_failed'}
                res = installer.install_archive(str(dl_path), confirm=True)
                if res.get('ok'):
                    installer.set_release_info({
                        'source': 'github',
                        'release_tag': update_info.get('version'),
                        'asset_url': download_url,
                        'asset_updated_at': update_info.get('asset_updated_at', ''),
                        'updated_at': time.time(),
                    })
                    clean_legacy_pet_installations(installer.target)
                    if was_running:
                        launcher.start()
                    return {'ok': True, 'updated': True, 'install_path': str(installer.target), 'version': update_info.get('version'), 'source': 'github'}
                return res

        return {'ok': False, 'code': 'unknown_update_source'}

    finally:
        if was_running and not launcher.status().get('running', False) and launcher.status().get('available'):
            launcher.start()
