use std::{env, process::Command};
fn llvm(config: &str, args: &[&str]) -> String {
    let output = Command::new(config)
        .args(args)
        .output()
        .expect("LLVM_CONFIG must point to llvm-config");
    assert!(
        output.status.success(),
        "llvm-config failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn main() {
    println!("cargo:rerun-if-env-changed=LLVM_CONFIG");
    println!("cargo:rerun-if-changed=build.rs");
    let config = env::var("LLVM_CONFIG").unwrap_or_else(|_| "llvm-config".into());
    let version = llvm(&config, &["--version"]);
    let major: u32 = version.split('.').next().unwrap().parse().unwrap();
    assert!(
        (14..=18).contains(&major),
        "This backend supports LLVM 14-18; found {version}"
    );
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ARCH").unwrap(),
        "x86_64",
        "The initial JIT backend targets x86_64"
    );
    let libdir = llvm(&config, &["--libdir"]);
    println!("cargo:rustc-link-search=native={libdir}");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{libdir}");
    for name in llvm(&config, &["--link-shared", "--libnames"]).split_whitespace() {
        let name = name.strip_prefix("lib").expect("LLVM library name");
        let name = name.split(".so").next().unwrap();
        println!("cargo:rustc-link-lib=dylib={name}");
    }
    println!("cargo:rustc-env=PRIM_LLVM_VERSION={version}");
    println!("cargo:rustc-env=PRIM_LLVM_MAJOR={major}");
}
