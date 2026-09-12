# -*- coding: utf-8 -*-
"""Optional desktop-pet runtime.

This package intentionally contains no Cubism Core, renderer, model, or
third-party character art.  A release can enable a model only after the bundle
passes :func:`verify_model_bundle` and platform evidence is attached.
"""

from .controller import PetController
from .fullscreen import foreground_fullscreen
from .hermes_adapter import (
    HermesPetBridge,
    HermesPetLauncher,
    HermesPetPluginInstaller,
    get_app_install_dir,
    get_default_pet_install_root,
)
from .model_manifest import verify_model_bundle
from .task_queue import PetBatchQueue
from .window_adapter import NativePetProbe, PetProbeDragBridge
from .store import (
    BUILTIN_PETS,
    InstalledPet,
    PetStoreError,
    get_builtin_pets,
    list_pets,
    register_local_pet,
    remove_pet,
    slugify,
)
from .updater import (
    apply_pet_update,
    check_pet_update,
    clean_legacy_pet_installations,
)

__all__ = [
    "BUILTIN_PETS",
    "NativePetProbe",
    "HermesPetBridge",
    "HermesPetLauncher",
    "HermesPetPluginInstaller",
    "get_app_install_dir",
    "get_default_pet_install_root",
    "apply_pet_update",
    "check_pet_update",
    "clean_legacy_pet_installations",
    "PetBatchQueue",
    "PetController",
    "PetProbeDragBridge",
    "foreground_fullscreen",
    "verify_model_bundle",
    "InstalledPet",
    "PetStoreError",
    "get_builtin_pets",
    "list_pets",
    "register_local_pet",
    "remove_pet",
    "slugify",
]
