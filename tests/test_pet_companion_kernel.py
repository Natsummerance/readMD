import json
from concurrent.futures import ThreadPoolExecutor

import pytest

from src.readmd_modules.pet import PetCompanion, PetController, HermesPetBridge


def test_interactions_persist_independently_for_each_character(tmp_path):
    now = [1000.0]
    pet = PetCompanion(tmp_path, clock=lambda: now[0])
    fed = pet.interact('hermes', 'feed')['companion']
    assert fed['energy'] == 95 and fed['affection'] == 1
    played = pet.interact('hermes', 'play')['companion']
    assert played['energy'] == 85 and played['xp'] == 13
    assert pet.snapshot('live2d:arch-chan')['xp'] == 0
    restored = PetCompanion(tmp_path, clock=lambda: now[0])
    assert restored.snapshot('hermes')['xp'] == 13
    assert restored.interact('hermes', 'feed')['code'] == 'pet_action_cooldown'
    now[0] += 31
    assert restored.interact('hermes', 'feed')['ok']


def test_rest_recovers_energy_and_wake_stops_recovery(tmp_path):
    now = [1000.0]
    pet = PetCompanion(tmp_path, clock=lambda: now[0])
    pet.interact('hermes', 'play')
    pet.interact('hermes', 'rest')
    now[0] += 300
    assert pet.snapshot('hermes')['energy'] == 80
    pet.interact('hermes', 'wake')
    now[0] += 300
    assert pet.snapshot('hermes')['energy'] == 80


def test_concurrent_feeds_do_not_bypass_cooldown(tmp_path):
    pet = PetCompanion(tmp_path, clock=lambda: 1000)
    with ThreadPoolExecutor(max_workers=8) as workers:
        results = list(workers.map(lambda _: pet.interact('hermes', 'feed'), range(20)))
    assert sum(result['ok'] for result in results) == 1
    assert pet.snapshot('hermes')['xp'] == 3


@pytest.mark.parametrize('action', [None, [], {}, 'run-shell'])
def test_unknown_action_is_rejected(tmp_path, action):
    with pytest.raises(ValueError, match='invalid_pet_action'):
        PetCompanion(tmp_path).interact('hermes', action)


def test_work_activity_survives_hiding_and_completion_expires():
    now = [1000.0]
    pet = PetController(enabled=True, clock=lambda: now[0])
    pet.handle_event('work_started', 'a')
    pet.handle_event('work_started', 'b')
    pet.set_fullscreen(True)
    pet.handle_event('work_succeeded', 'a')
    assert pet.set_fullscreen(False)['state'] == 'busy'
    assert pet.snapshot()['active_tasks'] == 1
    pet.handle_event('work_failed', 'b')
    assert pet.snapshot()['state'] == 'error'
    now[0] += 6
    assert pet.snapshot()['state'] == 'idle'
    assert pet.snapshot()['fps_cap'] == 6
    revision = pet.snapshot()['revision']
    pet.handle_event('work_failed', 'b')
    assert pet.snapshot()['revision'] == revision


def test_parallel_tasks_do_not_erase_each_other():
    pet = PetController(enabled=True)
    with ThreadPoolExecutor(max_workers=8) as workers:
        list(workers.map(lambda key: pet.handle_event('work_started', str(key)), range(100)))
    assert pet.snapshot()['active_tasks'] == 100
    with ThreadPoolExecutor(max_workers=8) as workers:
        list(workers.map(lambda key: pet.handle_event('work_succeeded', str(key)), range(100)))
    assert pet.snapshot()['active_tasks'] == 0


def test_durable_commands_are_fifo_and_invalid_entry_does_not_block(tmp_path):
    bridge = HermesPetBridge(tmp_path)
    bridge.commands_dir.mkdir(parents=True)
    (bridge.commands_dir / '00.json').write_text('{broken')
    for n, action in enumerate(['feed', 'play', 'rest'], 1):
        (bridge.commands_dir / f'{n:02}.json').write_text(json.dumps({'command': {'type': 'interact', 'action': action}}))
    assert [bridge.take_command()['action'] for _ in range(3)] == ['feed', 'play', 'rest']
    assert bridge.take_command() is None
    assert not list(bridge.commands_dir.glob('*.json'))


def test_identical_snapshots_do_not_rewrite_file(tmp_path):
    bridge = HermesPetBridge(tmp_path)
    bridge.publish({'visible': True, 'state': 'idle'})
    stamp = bridge.state_path.stat().st_mtime_ns
    bridge.publish({'visible': True, 'state': 'idle'})
    assert bridge.state_path.stat().st_mtime_ns == stamp


@pytest.mark.parametrize('value', [float('inf'), float('nan'), 'bad'])
def test_bad_bounds_do_not_crash_command_consumer(tmp_path, value):
    bridge = HermesPetBridge(tmp_path)
    bridge.commands_dir.mkdir(parents=True)
    (bridge.commands_dir / '1.json').write_text(json.dumps({'command': {'type': 'bounds', 'bounds': {'x': value}}}))
    assert bridge.take_command() is None
