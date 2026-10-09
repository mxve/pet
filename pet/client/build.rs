/*!
build script:
  bake pets and server key into the client
*/

use std::env;
use std::fs;
use std::path::PathBuf;

/// bake pets so we can ship a single artifact
fn main() {
    println!("cargo::rerun-if-changed=pets");
    embed_server_key();
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"));
    let mut pets: Vec<PathBuf> = fs::read_dir(root.join("pets"))
        .expect("the pets folder exists")
        .map(|entry| entry.expect("the pets folder is readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "toml"))
        .collect();
    pets.sort();
    let sources: String = pets.iter().map(|path| format!("    include_str!({path:?}),\n")).collect();
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets the out dir"));
    fs::write(out.join("pets.rs"), format!("const BUILTIN: &[&str] = &[\n{sources}];\n")).expect("the out dir is writable");
}

/// the key file comes from the cargo config, so nobody passes keys by hand
fn embed_server_key() {
    println!("cargo::rerun-if-env-changed=PET_SERVER_KEY");
    println!("cargo::rerun-if-env-changed=PET_SERVER_KEY_FILE");
    if env::var_os("PET_SERVER_KEY").is_some() {
        return;
    }
    let Ok(path) = env::var("PET_SERVER_KEY_FILE") else {
        return;
    };
    println!("cargo::rerun-if-changed={path}");
    if let Ok(key) = fs::read_to_string(&path) {
        println!("cargo::rustc-env=PET_SERVER_KEY={}", key.trim());
    }
}
