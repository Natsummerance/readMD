# -*- coding: utf-8 -*-
"""Offline Hermes-compatible local pet store contracts."""

import json

import pytest

from src.readmd_modules.pet.store import PetStoreError, list_pets, register_local_pet, remove_pet


PNG = b"\x89PNG\r\n\x1a\n" + b"readmd-test"


def test_register_list_and_remove_pet_uses_hermes_layout(tmp_path):
    pet = register_local_pet(tmp_path, slug="Arch Chan", spritesheet=PNG,
                             display_name="Arch Chan", description="offline mascot")
    assert pet.slug == "arch-chan"
    assert (tmp_path / "pets" / "arch-chan" / "pet.json").is_file()
    assert list_pets(tmp_path)[0].as_dict()["display_name"] == "Arch Chan"
    assert remove_pet(tmp_path, "arch-chan") is True
    assert list_pets(tmp_path) == []


def test_invalid_slug_and_conflict_are_rejected_without_escape(tmp_path):
    with pytest.raises(PetStoreError) as error:
        register_local_pet(tmp_path, slug="../outside", spritesheet=PNG)
    assert error.value.code == "pet_slug_invalid"
    register_local_pet(tmp_path, slug="demo", spritesheet=PNG)
    with pytest.raises(PetStoreError) as error:
        register_local_pet(tmp_path, slug="demo", spritesheet=PNG)
    assert error.value.code == "pet_already_exists"
    assert not (tmp_path / "outside").exists()


def test_corrupt_or_oversized_sprite_is_rejected(tmp_path):
    with pytest.raises(PetStoreError) as error:
        register_local_pet(tmp_path, slug="demo", spritesheet=b"not-an-image")
    assert error.value.code == "pet_spritesheet_format_invalid"


def test_list_ignores_symlink_or_malformed_entries(tmp_path):
    root = tmp_path / "pets"
    root.mkdir()
    malformed = root / "broken"
    malformed.mkdir()
    (malformed / "pet.json").write_text(json.dumps({"id": "broken"}), encoding="utf-8")
    assert list_pets(tmp_path) == []


def test_builtin_pets_protection_and_listing(tmp_path):
    builtins = list_pets(tmp_path, include_builtins=True)
    slugs = [p.slug for p in builtins]
    assert "mochi" in slugs
    assert "moss" in slugs
    assert "amber" in slugs
    assert all(p.is_builtin for p in builtins if p.slug in ("mochi", "moss", "amber"))

    with pytest.raises(PetStoreError) as exc:
        remove_pet(tmp_path, "mochi")
    assert exc.value.code == "pet_cannot_delete_builtin"

