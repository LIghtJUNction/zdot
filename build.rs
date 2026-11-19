fn main() {
    println!("cargo:rustc-link-arg-bin=zdot=-nostdlib");
    println!("cargo:rustc-link-arg-bin=zdot=-Wl,-static");
    println!("cargo:rustc-link-arg-bin=zdot=-Wl,-no-pie");
    println!("cargo:rustc-link-arg-bin=zdot=-Wl,--no-dynamic-linker");
    println!("cargo:rustc-link-arg-bin=zdot=-Wl,--build-id=none");
}
