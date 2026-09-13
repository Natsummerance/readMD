"""Persistent, offline companionship mechanics shared by every pet renderer."""
from __future__ import annotations

import copy
import json
import math
import os
import threading
import time
from pathlib import Path


class PetCompanion:
    ACTIONS = {'pet', 'feed', 'play', 'rest', 'wake'}
    COOLDOWNS = {'pet': 2, 'feed': 30, 'play': 20, 'rest': 0, 'wake': 0}

    def __init__(self, data_dir, *, clock=time.time):
        self.path = Path(data_dir) / 'pet' / 'companion.json'
        self._clock = clock
        self._lock = threading.RLock()
        self._profiles = {}
        try:
            if self.path.stat().st_size <= 1024 * 1024:
                raw = json.loads(self.path.read_text(encoding='utf-8'))
                if isinstance(raw, dict) and raw.get('version') == 1 and isinstance(raw.get('profiles'), dict):
                    self._profiles = dict(list(raw['profiles'].items())[:256])
        except (OSError, ValueError, TypeError):
            pass

    @staticmethod
    def _number(value, default, low=0, high=100):
        try:
            number = float(value)
            return max(low, min(high, number)) if math.isfinite(number) else default
        except (ValueError, TypeError):
            return default

    def _profile(self, character):
        if not isinstance(character, str) or not character or len(character) > 128:
            raise ValueError('invalid_pet_character')
        now = self._clock()
        raw = self._profiles.get(character, {})
        raw = raw if isinstance(raw, dict) else {}
        profile = {
            'energy': self._number(raw.get('energy'), 80),
            'mood': self._number(raw.get('mood'), 75),
            'affection': self._number(raw.get('affection'), 0),
            'xp': int(self._number(raw.get('xp'), 0, high=1_000_000)),
            'resting': raw.get('resting') is True,
            'updated_at': self._number(raw.get('updated_at'), now, high=max(now, 0)),
            'last_actions': raw.get('last_actions') if isinstance(raw.get('last_actions'), dict) else {},
            'revision': int(self._number(raw.get('revision'), 0, high=1_000_000_000)),
            'last_action': str(raw.get('last_action') or ''),
        }
        elapsed = max(0, min(86400, now - profile['updated_at']))
        if elapsed >= 60:
            # No punishment for closing the app: only resting restores energy.
            if profile['resting']:
                profile['energy'] = min(100, profile['energy'] + elapsed / 60 * 2)
            profile['updated_at'] = now
        if character not in self._profiles and len(self._profiles) >= 256:
            raise ValueError('pet_character_capacity')
        self._profiles[character] = profile
        return profile

    def snapshot(self, character):
        with self._lock:
            profile = copy.deepcopy(self._profile(character))
            profile['level'] = 1 + profile['xp'] // 50
            profile['character'] = character
            profile['energy'] = round(profile['energy'])
            profile['mood'] = round(profile['mood'])
            profile['cooldowns'] = {
                key: max(0, math.ceil(self._number(profile['last_actions'].get(key), 0, high=self._clock()) + seconds - self._clock()))
                for key, seconds in self.COOLDOWNS.items()
            }
            return profile

    def interact(self, character, action):
        if not isinstance(action, str) or action not in self.ACTIONS:
            raise ValueError('invalid_pet_action')
        with self._lock:
            profile = self._profile(character)
            wait = self.snapshot(character)['cooldowns'][action]
            profile = self._profiles[character]
            if wait:
                return {'ok': False, 'code': 'pet_action_cooldown', 'retry_after': wait, 'companion': self.snapshot(character)}
            if action == 'play' and profile['energy'] < 10:
                return {'ok': False, 'code': 'pet_needs_rest', 'companion': self.snapshot(character)}
            if action == 'feed':
                profile['energy'] = min(100, profile['energy'] + 15)
                profile['mood'] = min(100, profile['mood'] + 4)
            elif action == 'play':
                profile['energy'] -= 10
                profile['mood'] = min(100, profile['mood'] + 12)
                profile['resting'] = False
            elif action == 'pet':
                profile['mood'] = min(100, profile['mood'] + 6)
            else:
                profile['resting'] = action == 'rest'
            if action in {'pet', 'feed', 'play'}:
                profile['affection'] = min(100, profile['affection'] + 1)
                profile['xp'] = min(1_000_000, profile['xp'] + (10 if action == 'play' else 3))
            profile['revision'] += 1
            profile['last_action'] = action
            profile['last_actions'][action] = self._clock()
            profile['updated_at'] = self._clock()
            self._save()
            return {'ok': True, 'companion': self.snapshot(character)}

    def _save(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temporary = self.path.with_suffix('.tmp')
        temporary.write_text(json.dumps({'version': 1, 'profiles': self._profiles}, ensure_ascii=False, allow_nan=False), encoding='utf-8')
        try:
            os.replace(temporary, self.path)
        finally:
            temporary.unlink(missing_ok=True)
