use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=pets");
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"));
    let mut pets: Vec<PathBuf> = fs::read_dir(root.join("pets"))
        .expect("the pets folder exists")
        .map(|entry| entry.expect("the pets folder is readable").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    pets.sort();
    let sources: String = pets
        .iter()
        .map(|path| format!("    include_str!({path:?}),\n"))
        .collect();
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets the out dir"));
    fs::write(
        out.join("pets.rs"),
        format!("const BUILTIN: &[&str] = &[\n{sources}];\n"),
    )
    .expect("the out dir is writable");
}
