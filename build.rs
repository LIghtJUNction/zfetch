fn main() {
    for arg in [
        "-nostdlib",
        "-Wl,-static",
        "-Wl,-no-pie",
        "-Wl,--no-dynamic-linker",
        "-Wl,--build-id=none",
    ] {
        println!("cargo:rustc-link-arg-bin=zfetch={arg}");
    }
}
