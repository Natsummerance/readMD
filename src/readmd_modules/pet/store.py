# -*- coding: utf-8 -*-
"""ReadMD's offline local-pet store, aligned with Hermes' file contract.

Hermes stores each pet as ``pets/<slug>/pet.json`` plus a spritesheet.  The
store deliberately contains no downloader: importing a local bundle is an
explicit, bounded operation and petdex/network discovery remains outside the
release-critical path.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import tempfile
from dataclasses import dataclass
from pathlib import Path


class PetStoreError(ValueError):
    """A safe, stable local-pet import error."""

    def __init__(self, code: str):
        self.code = str(code or "pet_store_error")
        super().__init__(self.code)


_SLUG = re.compile(r"^[a-z0-9][a-z0-9-]{0,62}$")
_MAX_SPRITESHEET = 20 * 1024 * 1024
_MAX_META = 64 * 1024
_PNG = b"\x89PNG\r\n\x1a\n"
_RIFF = b"RIFF"
_WEBP = b"WEBP"


@dataclass(frozen=True)
class InstalledPet:
    slug: str
    display_name: str
    description: str
    directory: str
    spritesheet: str
    sha256: str
    is_builtin: bool = False

    def as_dict(self) -> dict:
        return {
            "slug": self.slug,
            "display_name": self.display_name,
            "description": self.description,
            "directory": self.directory,
            "spritesheet": self.spritesheet,
            "sha256": self.sha256,
            "is_builtin": self.is_builtin,
        }


BUILTIN_PETS = {
    "mochi": {
        "slug": "mochi",
        "display_name": "糯米 / Mochi",
        "description": "奶油白的小猫，焦糖色耳尖与尾巴，系一条鼠尾草绿围巾。",
        "filename": "mochi-sprite.png",
    },
    "moss": {
        "slug": "moss",
        "display_name": "苔苔 / Moss",
        "description": "圆润的浅绿色史莱姆，头顶两片嫩叶，深墨绿豆豆眼。",
        "filename": "moss-sprite.png",
    },
    "amber": {
        "slug": "amber",
        "display_name": "琥珀 / Amber",
        "description": "橘色小狐狸，奶白胸口与尾尖，短腿大尾巴。",
        "filename": "amber-sprite.png",
    },
}


def get_builtin_pet_asset_path(slug: str) -> Path | None:
    item = BUILTIN_PETS.get(slug)
    if not item:
        return None
    root = Path(__file__).resolve().parents[3]
    asset = root / "assets" / "pet" / item["filename"]
    if asset.is_file():
        return asset
    cwd_asset = Path("assets/pet") / item["filename"]
    if cwd_asset.is_file():
        return cwd_asset.resolve()
    return None


def get_builtin_pets() -> list[InstalledPet]:
    res: list[InstalledPet] = []
    for slug, info in BUILTIN_PETS.items():
        path = get_builtin_pet_asset_path(slug)
        if path and path.is_file():
            res.append(InstalledPet(
                slug=slug,
                display_name=info["display_name"],
                description=info["description"],
                directory=str(path.parent),
                spritesheet=str(path),
                sha256=_sha256(path),
                is_builtin=True,
            ))
    return res


def _root(data_dir: os.PathLike[str] | str) -> Path:
    root = Path(data_dir).expanduser().resolve() / "pets"
    root.mkdir(parents=True, exist_ok=True)
    return root


def slugify(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", str(value or "").strip().lower()).strip("-")
    return slug[:63] if slug else "pet"


def _safe_slug(value: str) -> str:
    slug = str(value or "").strip().lower()
    if not _SLUG.fullmatch(slug):
        raise PetStoreError("pet_slug_invalid")
    return slug


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _spritesheet_path(directory: Path, metadata: dict) -> Path | None:
    declared = str(metadata.get("spritesheetPath") or metadata.get("spritesheet_path") or "").strip()
    names = [declared] if declared else []
    names.extend(("spritesheet.webp", "spritesheet.png", "sprite.webp", "sprite.png"))
    for name in names:
        candidate = (directory / name).resolve()
        if candidate.parent != directory.resolve() or candidate.is_symlink():
            continue
        if candidate.is_file() and candidate.suffix.lower() in {".webp", ".png"}:
            return candidate
    return None


def list_pets(data_dir: os.PathLike[str] | str, include_builtins: bool = False) -> list[InstalledPet]:
    root = _root(data_dir)
    result: list[InstalledPet] = []
    if include_builtins:
        result.extend(get_builtin_pets())
    for directory in sorted(root.iterdir(), key=lambda item: item.name):
        if not directory.is_dir() or directory.is_symlink():
            continue
        try:
            slug = _safe_slug(directory.name)
            meta_path = directory / "pet.json"
            if not meta_path.is_file() or meta_path.stat().st_size > _MAX_META:
                continue
            metadata = json.loads(meta_path.read_text(encoding="utf-8"))
            if not isinstance(metadata, dict):
                continue
            sprite = _spritesheet_path(directory, metadata)
            if not sprite or sprite.stat().st_size > _MAX_SPRITESHEET:
                continue
            result.append(InstalledPet(
                slug=slug,
                display_name=str(metadata.get("displayName") or metadata.get("display_name") or slug),
                description=str(metadata.get("description") or ""),
                directory=str(directory),
                spritesheet=str(sprite),
                sha256=_sha256(sprite),
            ))
        except (OSError, ValueError, TypeError, json.JSONDecodeError):
            continue
    return result


def _atomic_write(path: Path, data: bytes) -> None:
    fd, temp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=str(path.parent))
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp_name, path)
    finally:
        try:
            os.unlink(temp_name)
        except OSError:
            pass


def register_local_pet(
    data_dir: os.PathLike[str] | str,
    *,
    slug: str,
    spritesheet: bytes,
    display_name: str = "",
    description: str = "",
    replace: bool = False,
) -> InstalledPet:
    """Register a bounded PNG/WebP bundle using Hermes' on-disk layout."""
    raw_slug = str(slug or "").strip()
    if not raw_slug or "/" in raw_slug or "\\" in raw_slug or raw_slug in {".", ".."}:
        raise PetStoreError("pet_slug_invalid")
    slug = slugify(raw_slug)
    if not _SLUG.fullmatch(slug):
        raise PetStoreError("pet_slug_invalid")
    if not isinstance(spritesheet, (bytes, bytearray)) or not spritesheet:
        raise PetStoreError("pet_spritesheet_missing")
    raw = bytes(spritesheet)
    if len(raw) > _MAX_SPRITESHEET:
        raise PetStoreError("pet_spritesheet_too_large")
    if not (raw.startswith(_PNG) or (raw[:4] == _RIFF and raw[8:12] == _WEBP)):
        raise PetStoreError("pet_spritesheet_format_invalid")
    try:
        from .sprite_processor import normalize_and_segment_spritesheet
        raw = normalize_and_segment_spritesheet(raw)
    except Exception:
        pass
    root = _root(data_dir)
    directory = root / slug
    # Never write through a pre-existing symlink.  ``mkdir(exist_ok=True)``
    # accepts a symlink to a directory on some platforms, which would let an
    # imported bundle escape the pet store even though the slug itself is safe.
    if directory.is_symlink() or (directory.exists() and not directory.is_dir()):
        raise PetStoreError("pet_destination_invalid")
    if directory.exists() and not replace:
        raise PetStoreError("pet_already_exists")
    directory.mkdir(parents=True, exist_ok=True)
    sprite_name = "spritesheet.png" if raw.startswith(_PNG) else "spritesheet.webp"
    _atomic_write(directory / sprite_name, raw)
    metadata = {
        "id": slug,
        "displayName": str(display_name or slug)[:200],
        "description": str(description or "")[:2000],
        "spritesheetPath": sprite_name,
        "createdBy": "readmd-local-import",
    }
    _atomic_write(directory / "pet.json", json.dumps(metadata, ensure_ascii=False, indent=2).encode("utf-8"))
    pets = [item for item in list_pets(data_dir) if item.slug == slug]
    if not pets:
        raise PetStoreError("pet_import_invalid")
    return pets[0]


def remove_pet(data_dir: os.PathLike[str] | str, slug: str) -> bool:
    slug = _safe_slug(slug)
    if slug in BUILTIN_PETS:
        raise PetStoreError("pet_cannot_delete_builtin")
    root = _root(data_dir)
    directory = (root / slug).resolve()
    if directory.parent != root.resolve() or not directory.is_dir() or directory.is_symlink():
        raise PetStoreError("pet_not_found")
    # Only remove a directory that follows the local pet contract.  This
    # avoids turning the API into an arbitrary recursive-delete primitive.
    if not (directory / "pet.json").is_file():
        raise PetStoreError("pet_not_found")
    import shutil
    shutil.rmtree(directory)
    return True


__all__ = [
    "BUILTIN_PETS",
    "InstalledPet",
    "PetStoreError",
    "get_builtin_pet_asset_path",
    "get_builtin_pets",
    "list_pets",
    "register_local_pet",
    "remove_pet",
    "slugify",
]
