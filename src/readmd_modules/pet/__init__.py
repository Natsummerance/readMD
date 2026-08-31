# -*- coding: utf-8 -*-
"""Optional desktop-pet runtime.

This package intentionally contains no Cubism Core, renderer, model, or
third-party character art.  A release can enable a model only after the bundle
passes :func:`verify_model_bundle` and platform evidence is attached.
"""

from .controller import PetController
from .model_manifest import verify_model_bundle
from .task_queue import PetBatchQueue
from .window_adapter import NativePetProbe, PetProbeDragBridge

__all__ = [
    "NativePetProbe",
    "PetBatchQueue",
    "PetController",
    "PetProbeDragBridge",
    "verify_model_bundle",
]
