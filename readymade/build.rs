use std::env;

fn main() {
    let out_dir = env::var("TARGET_DIR").unwrap();
    println!("cargo:rustc-link-search={}", out_dir);
}