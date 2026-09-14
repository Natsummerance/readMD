//! Offline Phase 0 smoke entry point.  It exercises only the protocol core.

use readmd_pet_rust::{AppliedState, PetSnapshot, SerializedRect, SNAPSHOT_FORMAT_VERSION};

fn main() {
    let snapshot = PetSnapshot {
        format_version: SNAPSHOT_FORMAT_VERSION,
        visible: true,
        fullscreen: false,
        generation: 1,
        renderer: Some("live2d".to_owned()),
        bounds: Some(SerializedRect { x: 0.0, y: 0.0, width: 300.0, height: 420.0 }),
    };
    let mut applied = AppliedState::default();
    let accepted = applied.apply(&snapshot).expect("fixture is valid");
    println!("{{\"phase\":\"phase0\",\"accepted\":{accepted},\"production_host\":false}}");
}
