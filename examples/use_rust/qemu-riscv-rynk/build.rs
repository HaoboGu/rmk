use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Copy memory.x to OUT_DIR so the linker can find it.
    for (name, bytes) in [("memory.x", include_bytes!("memory.x").as_slice())] {
        File::create(out.join(name)).unwrap().write_all(bytes).unwrap();
        println!("cargo:rerun-if-changed={name}");
    }
    println!("cargo:rustc-link-search={}", out.display());

    // Link order: our memory.x first, then riscv-rt's link.x.
    println!("cargo:rustc-link-arg-bins=-Tmemory.x");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
}
