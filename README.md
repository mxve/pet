# ascii pet

## install
- make sure rust is installed -> https://rustup.rs
- package installation
  - install package
    - `cargo install --force --index sparse+https://gitea.treecom.local/api/packages/Emilia-Sophie.Ude/cargo/ pet`
  - run binary from anywhere with `pet`
- OR
  - build using cargo
    - clone repo
      - `git clone https://gitea.treecom.local/Emilia-Sophie.Ude/pet.git`
    - build release binary
      - `cargo build --release`
      - binary is in [workdir]/target/release

## dev mode
- `cargo run -- --dev` -> no save, 600x speed, debug overlay, ctrl dev keys

## controls
- esc/q -> quit