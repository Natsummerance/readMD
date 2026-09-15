use readmd_pet_rust::runtime::{HostConfig, PetHost};

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("readmd-pet-rust {} protocol=1", env!("CARGO_PKG_VERSION"));
        return;
    }
    let config = match HostConfig::from_env() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("readmd-pet-rust: {error}");
            std::process::exit(2);
        }
    };
    if let Err(error) = PetHost::run(config) {
        eprintln!("readmd-pet-rust: {error}");
        std::process::exit(1);
    }
}
