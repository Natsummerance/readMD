# -*- coding: utf-8 -*-
"""
update_tuple_registry.py
Updates docs/architecture/pet-rust/tuple-registry.json
"""
import json, pathlib

repo = pathlib.Path(__file__).resolve().parents[1]
registry_path = repo / 'docs' / 'architecture' / 'pet-rust' / 'tuple-registry.json'

tuples = json.loads(registry_path.read_text(encoding='utf-8'))
by_id = {t['display_id']: t for t in tuples}

# T-18: fedora-42-gnome50
t = by_id['T-18']
t['os_build'] = 'Standard'
t['support_role'] = 'legacy_supported'
t['fact_checked_at'] = '2026-09-14'
t['fact_source'] = 'Fedora Release Schedule 2026'

# T-17: fedora-42-kde6
t = by_id['T-17']
t['os_build'] = 'Standard'
t['support_role'] = 'legacy_supported'
t['fact_checked_at'] = '2026-09-14'
t['fact_source'] = 'Fedora Release Schedule 2026'

# T-16: fedora-40-kde6
t = by_id['T-16']
t['support_role'] = 'legacy_supported'
t['fact_checked_at'] = '2026-09-14'
t['fact_source'] = 'Fedora Release Schedule 2026'

# T-01: windows-11-24h2-x64
t = by_id['T-01']
t['support_role'] = 'legacy_supported'
t['fact_checked_at'] = '2026-09-14'
t['fact_source'] = 'Microsoft Windows Servicing Matrix 2026'

# T-03: windows-11-24h2-arm64
t = by_id['T-03']
t['support_role'] = 'legacy_supported'
t['fact_checked_at'] = '2026-09-14'
t['fact_source'] = 'Microsoft Windows Servicing Matrix 2026'

# T-19: archlinux-rolling-sway
t = by_id['T-19']
t['snapshot_date'] = '2026-09-14'
t['os_build_snapshot_id'] = 'archlinux-2026.09.01-x86_64.iso'
t['support_role'] = 'current_representative'
t['fact_checked_at'] = '2026-09-14'
t['fact_source'] = 'Arch Linux Rolling Release'

# T-20: archlinux-rolling-hyprland
t = by_id['T-20']
t['snapshot_date'] = '2026-09-14'
t['os_build_snapshot_id'] = 'archlinux-2026.09.01-x86_64.iso'
t['support_role'] = 'current_representative'
t['fact_checked_at'] = '2026-09-14'
t['fact_source'] = 'Arch Linux Rolling Release'

# T-12: ubuntu-24.04-gnome46
t = by_id['T-12']
t['support_role'] = 'current_representative'
t['fact_checked_at'] = '2026-09-14'
t['fact_source'] = 'Ubuntu LTS 24.04 Release'

# Append T-25 through T-28
new_tuples = [
  {"tuple_key": "fedora-44-gnome50-x64-wayland-gnomecompanion", "display_id": "T-25", "os": "Fedora", "os_version": "44", "os_build": "Workstation", "arch": "x86_64", "desktop_environment": "GNOME", "desktop_version": "50", "display_protocol": "Wayland", "compositor": "Mutter", "planned_backend": "GnomeCompanionBackend", "backend_binding_status": "Planned", "lifecycle": "Planned", "is_phase0_representative": True, "support_role": "current_representative", "fact_checked_at": "2026-09-14", "fact_source": "Fedora 44 Workstation Release 2026"},
  {"tuple_key": "fedora-44-kde66-x64-wayland-layershell", "display_id": "T-26", "os": "Fedora", "os_version": "44", "os_build": "KDE Plasma Desktop", "arch": "x86_64", "desktop_environment": "KDE", "desktop_version": "6.6", "display_protocol": "Wayland", "compositor": "KWin", "planned_backend": "LayerShellBackend", "backend_binding_status": "Planned", "lifecycle": "Planned", "is_phase0_representative": True, "support_role": "current_representative", "fact_checked_at": "2026-09-14", "fact_source": "Fedora KDE Plasma Desktop 44 Release 2026 / Plasma 6.6"},
  {"tuple_key": "windows-11-25h2-x64-dwm-win32", "display_id": "T-27", "os": "Windows", "os_version": "11", "os_build": "25H2", "arch": "x86_64", "desktop_environment": "DWM", "desktop_version": "10.0.27xx", "display_protocol": "DWM", "compositor": "DWM", "planned_backend": "Win32Backend", "backend_binding_status": "Planned", "lifecycle": "Planned", "is_phase0_representative": True, "support_role": "current_representative", "fact_checked_at": "2026-09-14", "fact_source": "Microsoft Windows 11 25H2 mainstream release 2026"},
  {"tuple_key": "windows-11-26h1-arm64-dwm-win32", "display_id": "T-28", "os": "Windows", "os_version": "11", "os_build": "26H1", "arch": "aarch64", "desktop_environment": "DWM", "desktop_version": "10.0.28xx", "display_protocol": "DWM", "compositor": "DWM", "planned_backend": "Win32Backend", "backend_binding_status": "Planned", "lifecycle": "Planned", "is_phase0_representative": True, "support_role": "current_representative", "fact_checked_at": "2026-09-14", "fact_source": "Microsoft Windows 11 26H1 new-device ARM64 release 2026"}
]

existing_ids = {t['display_id'] for t in tuples}
for nt in new_tuples:
    if nt['display_id'] in existing_ids:
        print(f"WARNING: {nt['display_id']} already exists, skipping.")
    else:
        tuples.append(nt)
        print(f"APPENDED {nt['display_id']} ({nt['tuple_key']})")

registry_path.write_text(json.dumps(tuples, indent=2, ensure_ascii=False), encoding='utf-8')
print(f'WROTE tuple-registry.json ({len(tuples)} tuples total)')
print('All registry updates applied successfully.')
