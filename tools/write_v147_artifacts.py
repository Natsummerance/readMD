# -*- coding: utf-8 -*-
import json, pathlib
repo = pathlib.Path(__file__).resolve().parents[1]
arch = repo / 'docs' / 'architecture' / 'pet-rust'
arch.mkdir(parents=True, exist_ok=True)

golden_contract = {
  "$schema": "schema/golden-contract.schema.json",
  "version": "v1.4.7",
  "golden_generation": "2026-09-14",
  "migration_golden_commit_sha": "4dcfd73ce81a14ace7e429791e0594bea47b24e5",
  "_commit_identity_note": "migration_golden_commit_sha is IMMUTABLE. Requires Explicit Golden Re-record ADR + Golden Differential review + human approval to change. architecture_spec_commit_sha and tested_commit_sha are set externally by CI attestation.",
  "vendor": "hermes-agent-pet",
  "pinned_revision": "fb27614addac115d55299bc6538ae112fd01f688",
  "upstream_repository": "https://github.com/NousResearch/hermes-agent",
  "behavior_inputs": [
    {"path": "third_party/hermes-agent-pet/apps/desktop/src/app/pet-overlay/pet-overlay-app.tsx", "role": "Upstream Sprite click-through, double-click, and event handling", "key_lines": "L132-L165, L187-L210", "byte_size": 6185, "sha256": "9b2c3d492d0f2adaaa4e2b7ac536e988dea1908208ee7a321fa23203b2c7357d", "provenance": "vendor_pinned_snapshot", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:third_party/hermes-agent-pet/apps/desktop/src/app/pet-overlay/pet-overlay-app.tsx"},
    {"path": "packages/readmd-hermes-pet-adapter/scripts/build.mjs", "role": "Build-time adaptation script patching setComposerOpen to control({type:'open-menu'}) and generating bundle", "key_lines": "L41-L58, L60-L77", "byte_size": 4661, "sha256": "59fe50c2d31841aadef9013d2b47b107f49dac62434cebb5d3106bb97ca51db2", "provenance": "readmd_adaptation_script", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:packages/readmd-hermes-pet-adapter/scripts/build.mjs"},
    {"path": "packages/readmd-hermes-pet-adapter/src/electron-main.ts", "role": "Main window lifecycle, context menu model, tray and clipboard capture; toggle-app IPC owner", "key_lines": "L87-L142, L165-L172, L195-L202, L226-L237", "byte_size": 14929, "sha256": "0a1b6473d155f8121d77d1463316a7968b0d973f76bb6080f4abb58de65a269d", "provenance": "readmd_host_adapter", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:packages/readmd-hermes-pet-adapter/src/electron-main.ts"},
    {"path": "packages/readmd-hermes-pet-adapter/src/preload.ts", "role": "Preload ABI context exposure (window.hermesDesktop.petOverlay + window.readmdPet)", "key_lines": "L1-L40", "byte_size": 1883, "sha256": "fafeb3c1e5241efe3c25646f4ec1cb818ca46a17e375f85e3e16710963df1179", "provenance": "readmd_host_adapter", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:packages/readmd-hermes-pet-adapter/src/preload.ts"},
    {"path": "packages/readmd-hermes-pet-adapter/src/bridge-transport.ts", "role": "Durable FIFO queue and SnapshotReader atomic reader", "key_lines": "L8-L29, L31-L53", "byte_size": 2276, "sha256": "7055deed1d644687fe8fc1a3adff39fba85185644903e334be6b502397100723", "provenance": "readmd_host_adapter", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:packages/readmd-hermes-pet-adapter/src/bridge-transport.ts"},
    {"path": "packages/readmd-hermes-pet-adapter/src/renderer.tsx", "role": "Frontend React mounting, state dispatch, and error boundary", "key_lines": "L6-L44", "byte_size": 2002, "sha256": "5bbbd06c222c572d75b68b10bb09e910a5e02e6f1e89475a9811d0934dccaa36", "provenance": "readmd_renderer", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:packages/readmd-hermes-pet-adapter/src/renderer.tsx"},
    {"path": "packages/readmd-hermes-pet-adapter/src/live2d/stage.ts", "role": "Live2D stage hit test and bounds checking", "key_lines": "L125-L135", "byte_size": 19525, "sha256": "bec994ed0a299fd7f05156f54cef6fa06da750f96f6f931a547313bd3e64522a", "provenance": "readmd_renderer", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:packages/readmd-hermes-pet-adapter/src/live2d/stage.ts"},
    {"path": "third_party/hermes-agent-pet/apps/desktop/electron/pet-overlay-ipc.ts", "role": "Upstream pet IPC protocol and window click-through control", "key_lines": "L1-L150", "byte_size": 5824, "sha256": "5c99fce416fece34d0fb66fdb662af0fb0169b9c4e8aae71977f9a46ac171d8d", "provenance": "vendor_pinned_snapshot", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:third_party/hermes-agent-pet/apps/desktop/electron/pet-overlay-ipc.ts"},
    {"path": "src/readmd_modules/pet/hermes_adapter.py", "role": "Python host orchestrator, lifecycle management, and FIFO response; FIFO consumer only - NOT toggle-app IPC owner", "key_lines": "L309-L322, L364-L390", "byte_size": 33603, "sha256": "2a2f09188d3f9f6f52ac9a0a0571d3a94eaf2e385949184ef24058f3ec5b03ee", "provenance": "readmd_core_python", "behavior_critical": True, "golden_source_read": "git show 4dcfd73ce81a14ace7e429791e0594bea47b24e5:src/readmd_modules/pet/hermes_adapter.py"}
  ],
  "build_inputs": [
    {"path": "packages/readmd-hermes-pet-adapter/vite.config.mjs", "role": "Vite bundling configuration for renderer", "byte_size": 1358, "sha256": "8a5ed317a59a567fa8660e887d3b4e6067375583f85cb6072f7090cfae7c17db", "provenance": "readmd_build_config", "behavior_critical": True},
    {"path": "packages/readmd-hermes-pet-adapter/package.json", "role": "Dependencies, build scripts, and package metadata", "byte_size": 709, "sha256": "ee63a91062219ea13672d4440246745f5eb821573a3007b30a7385e857780600", "provenance": "readmd_package_meta", "behavior_critical": True},
    {"path": "packages/readmd-hermes-pet-adapter/package-lock.json", "role": "Dependency lockfile ensuring reproducible toolchain", "byte_size": 120446, "sha256": "c354caaed6f277fbddad21b6e27d6e2de961f6cefa9a397f3e213a9ecf3efb4f", "provenance": "adapter_lockfile", "behavior_critical": False}
  ],
  "generated_inputs": [
    {"path": "packages/readmd-hermes-pet-adapter/dist/electron-main.cjs", "role": "Production bundled Electron main host", "byte_size": 20120, "sha256": "b5747182b5e883e2e89aad869affbd3a7d3a7b8f70642a4997d757adb569f921", "provenance": "esbuild_bundle", "behavior_critical": True},
    {"path": "packages/readmd-hermes-pet-adapter/dist/preload.cjs", "role": "Production bundled Preload script", "byte_size": 3014, "sha256": "50d95c7d2b62f3ba3198597c0a73cb53d4fa6c89d36534d59268a3a0fb8c08e0", "provenance": "esbuild_bundle", "behavior_critical": True},
    {"path": "packages/readmd-hermes-pet-adapter/dist/renderer/index.html", "role": "Production bundled Renderer HTML entry", "byte_size": 536, "sha256": "d9f1da3457bac312a2790d270f6052de02c87290a19d623ee5d23bfb378ccb9d", "provenance": "vite_bundle", "behavior_critical": True}
  ],
  "runtime_assets": [
    {"path": "packages/readmd-hermes-pet-adapter/assets/hermes-sprite.png", "role": "Default Hermes character sprite sheet", "byte_size": 180556, "sha256": "a5661b457de00b9a57570effcb7a3ecb8f6cb960b48c6633987a32542f2f58e0", "provenance": "static_asset", "behavior_critical": True},
    {"path": "packages/readmd-hermes-pet-adapter/assets/mochi-sprite.png", "role": "Mochi character sprite sheet", "byte_size": 171542, "sha256": "6e03b6065b5790b9ec860f13edcf930c902930456e5be5915b264eb225c68c01", "provenance": "static_asset", "behavior_critical": False},
    {"path": "packages/readmd-hermes-pet-adapter/assets/moss-sprite.png", "role": "Moss character sprite sheet", "byte_size": 165431, "sha256": "088f67906646a79d1bf8232d8bce324d5b000185ba52dcf12853c9ccd4a99af6", "provenance": "static_asset", "behavior_critical": False},
    {"path": "packages/readmd-hermes-pet-adapter/assets/amber-sprite.png", "role": "Amber character sprite sheet", "byte_size": 174921, "sha256": "fe9a84570db99898b20ac40e31012228b4959c84b636c166447a15f5cfa26bab", "provenance": "static_asset", "behavior_critical": False},
    {"path": "packages/readmd-hermes-pet-adapter/dist/models/arch-chan/arch chan model0.model3.json", "role": "Arch-Chan Live2D model definition", "byte_size": 689, "sha256": "5d1a05c2eadba5296d2e26f67da5369191a9ca0d124a5e9fcbb142dff578679c", "provenance": "static_asset", "behavior_critical": True}
  ],
  "supporting_evidence": [
    {"path": "packages/readmd-hermes-pet-adapter/src/pet-life.ts", "role": "Pet life companion state machine and attribute constants", "byte_size": 12123, "sha256": "21ef9bf62592d4d00a3b99cd3fd6f50944245fb26ac1f51cd3f3ad2db20d1dd5", "provenance": "readmd_supporting", "behavior_critical": False},
    {"path": "third_party/hermes-agent-pet/UPSTREAM.md", "role": "Upstream provenance documentation and revision record", "byte_size": 966, "sha256": "35076f31d0ea6d70df494d6b2fab258d6348a24706e707c373629fea75d51110", "provenance": "vendor_doc", "behavior_critical": False}
  ],
  "preload_abi": {
    "interface_name": "ReadMDPetPreloadABI",
    "_namespace_note": "window.__HERMES_PET__ does NOT exist in real preload.ts. True namespaces: window.hermesDesktop.petOverlay (overlay control) and window.readmdPet (file drop). Do not invent or document window.__HERMES_PET__.",
    "namespaces": {
      "window.hermesDesktop.petOverlay": {
        "open": {"signature": "open(request: OpenRequest) => void", "arity": 1, "result_shape": "void", "transport": "ipc", "channel": "pet-overlay:open"},
        "close": {"signature": "close() => void", "arity": 0, "result_shape": "void", "transport": "ipc", "channel": "pet-overlay:close"},
        "setBounds": {"signature": "setBounds(bounds: Bounds) => void", "arity": 1, "result_shape": "void", "transport": "ipc", "channel": "pet-overlay:set-bounds"},
        "setIgnoreMouse": {"signature": "setIgnoreMouse(ignore: boolean) => void", "arity": 1, "_arity_note": "No optional second argument. options? parameter does NOT exist in real source.", "result_shape": "void", "transport": "ipc", "channel": "pet-overlay:set-ignore-mouse"},
        "setFocusable": {"signature": "setFocusable(focusable: boolean) => void", "arity": 1, "result_shape": "void", "transport": "ipc", "channel": "pet-overlay:set-focusable"},
        "pushState": {"signature": "pushState(payload: unknown) => void", "arity": 1, "result_shape": "void", "transport": "ipc", "channel": "pet-overlay:push-state"},
        "control": {"signature": "control(payload: { action?: string; type?: string; [key: string]: unknown }) => void", "arity": 1, "result_shape": "void", "transport": "ipc", "channel": "pet-overlay:control"},
        "onState": {"signature": "onState(callback: (payload: any) => void) => () => void", "arity": 1, "result_shape": "unsubscribe_fn", "transport": "ipc_listener", "channel": "pet-overlay:state"},
        "onControl": {"signature": "onControl(callback: (payload: any) => void) => () => void", "arity": 1, "result_shape": "unsubscribe_fn", "transport": "ipc_listener", "channel": "pet-overlay:control-event"}
      },
      "window.readmdPet": {
        "dropFiles": {"signature": "dropFiles(files: File[]) => void", "arity": 1, "_type_note": "files is File[] (browser File objects), NOT string[]", "result_shape": "void", "transport": "ipc", "channel": "readmd:drop-files"}
      }
    }
  },
  "bounds_policies": {
    "host_snapshot_bounds": {
      "min_width": 240, "min_height": 300, "max_width": 640, "max_height": 720, "min_overlap_dip": 40,
      "placement_formula": {
        "x_expr": "primary.x + Math.max(12, primary.width - width - 24)",
        "y_expr": "primary.y + Math.max(12, primary.height - height - 24)",
        "near_origin_min_margin": 12,
        "far_edge_margin": 24,
        "_formula_note": "Margins must NOT reduce to negative values. min/max guards ensure position stays within display bounds."
      }
    },
    "renderer_interactive_bounds": {"min_width": 80, "min_height": 80, "max_width": 640, "max_height": 720, "live2d_auto_scale": True}
  },
  "golden_observable_contract": {
    "_note": "These are Electron host observable facts. They are GOLDEN. Rust target-equivalence strategies are recorded separately.",
    "parent_liveness_check": "process.kill(parentPid, 0) throws ESRCH => closePetOverlay() + app.exit(0)",
    "snapshot_reader_key": "ino:mtimeNs:ctimeNs:size",
    "toggle_app_ipc_source": "packages/readmd-hermes-pet-adapter/src/electron-main.ts",
    "_toggle_note": "hermes_adapter.py is FIFO consumer, NOT toggle-app IPC remapping owner",
    "poll_sequence": ["check parent liveness", "ensure bridgeFile", "SnapshotReader.read()", "if no new snapshot: return early", "latest = normalizeState(next)", "if visible=false && !fullscreen: close + return", "resolve renderer", "open or apply host bounds", "fullscreen: hide/showInactive", "if renderer changed: load + return", "host_publish_state"],
    "_normalize_note": "normalizeState MUST occur before visible/window reconciliation step"
  },
  "target_equivalence_strategy": {
    "_note": "These are Rust implementation candidates, NOT golden facts. Subject to VAL validation.",
    "parent_liveness_rust_candidate": "inherited pipe EOF => shutdown (SUBJECT TO VAL, not yet golden)",
    "snapshot_reader_rust_candidate": "ino:mtimeNs:ctimeNs:size on Windows Rust (CANDIDATE - subject to VAL)"
  }
}
(arch / 'golden-contract.json').write_text(json.dumps(golden_contract, indent=2, ensure_ascii=False), encoding='utf-8')
print('WROTE golden-contract.json')

preload_fixture = {
  "$schema": "../schema/preload-abi-fixture.schema.json",
  "_description": "Machine-verifiable Preload ABI structural fixture. Gate-Golden-ABI must compare this structure exactly, not substring match.",
  "fixture_version": "v1.4.7",
  "migration_golden_commit_sha": "4dcfd73ce81a14ace7e429791e0594bea47b24e5",
  "forbidden_namespaces": ["window.__HERMES_PET__"],
  "required_namespaces": ["window.hermesDesktop.petOverlay", "window.readmdPet"],
  "namespace_contracts": {
    "window.hermesDesktop.petOverlay": {
      "required_methods": ["open", "close", "setBounds", "setIgnoreMouse", "setFocusable", "pushState", "control", "onState", "onControl"],
      "method_contracts": {
        "open": {"arity": 1, "result_shape": "void", "forbidden_signatures": ["open(bounds, renderer?)", "open(bounds: Bounds, renderer?: string) => Promise<boolean>"]},
        "close": {"arity": 0, "result_shape": "void", "forbidden_signatures": ["close() => Promise<boolean>"]},
        "setBounds": {"arity": 1, "result_shape": "void"},
        "setIgnoreMouse": {"arity": 1, "result_shape": "void", "forbidden_arity": 2, "forbidden_signatures": ["setIgnoreMouse(ignore: boolean, options?: { forward?: boolean }) => void", "setIgnoreMouse(ignore, options?)"]},
        "setFocusable": {"arity": 1, "result_shape": "void"},
        "pushState": {"arity": 1, "result_shape": "void"},
        "control": {"arity": 1, "result_shape": "void"},
        "onState": {"arity": 1, "result_shape": "unsubscribe_fn"},
        "onControl": {"arity": 1, "result_shape": "unsubscribe_fn"}
      }
    },
    "window.readmdPet": {
      "required_methods": ["dropFiles"],
      "method_contracts": {
        "dropFiles": {"arity": 1, "result_shape": "void", "input_type": "File[]", "forbidden_input_types": ["string[]"], "forbidden_signatures": ["dropFiles(files: string[]) => void"]}
      }
    }
  }
}
(arch / 'preload-abi.fixture.json').write_text(json.dumps(preload_fixture, indent=2, ensure_ascii=False), encoding='utf-8')
print('WROTE preload-abi.fixture.json')

attestation_template = {
  "$schema": "../schema/attestation.schema.json",
  "_description": "CI post-commit attestation. Generated AFTER commit by CI pipeline. NOT a tracked source file. Records tested_commit_sha which is the immutable SHA of the commit being verified.",
  "attestation_version": "v1.4.7",
  "_generation_note": "This file is generated by CI after the commit is made. It is NOT part of the commit it describes. If committed back, it must state: 'report describes commit <SHA>'.",
  "tested_commit_sha": "__CI_WILL_FILL_THIS__",
  "architecture_spec_commit_sha": "__CI_WILL_FILL_THIS__",
  "migration_golden_commit_sha": "4dcfd73ce81a14ace7e429791e0594bea47b24e5",
  "tool_version": "v1.4.7",
  "generated_at": "__CI_WILL_FILL_THIS__",
  "artifact_root_sha256": "__CI_WILL_FILL_THIS__",
  "working_tree_clean_at_test": "__CI_WILL_FILL_THIS__",
  "verification_results": {
    "verify_golden_contract": "__CI_WILL_FILL_THIS__",
    "verify_registry_integrity": "__CI_WILL_FILL_THIS__",
    "verify_spec_consistency": "__CI_WILL_FILL_THIS__",
    "verify_report_consistency": "__CI_WILL_FILL_THIS__",
    "mutation_test_suite": "__CI_WILL_FILL_THIS__"
  },
  "phase_0_readiness": "BLOCKED_BY_EVIDENCE"
}
(arch / 'pet-architecture-attestation.json').write_text(json.dumps(attestation_template, indent=2, ensure_ascii=False), encoding='utf-8')
print('WROTE pet-architecture-attestation.json')
print('All artifacts written successfully.')
