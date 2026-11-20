use std::env;
use std::path::PathBuf;
use std::process::Command;

fn run(cmd: &str, args: &[&str]) {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to run {}: {}", cmd, e));
    assert!(status.success(), "{} failed with {}", cmd, status);
}

fn main() {
    println!("cargo:rerun-if-changed=vendor/FastTree.c");
    println!("cargo:rerun-if-changed=vendor/src/ft_compat.c");
    println!("cargo:rerun-if-changed=vendor/src/ft_reset.c");
    println!("cargo:rerun-if-changed=vendor/include");

    let target = env::var("TARGET").unwrap();
    if !target.starts_with("wasm32-") {
        // Nothing to link for host builds; the Rust API will still compile.
        return;
    }

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let cc = env::var("CC").unwrap_or_else(|_| "clang".to_string());
    let ar = env::var("AR").unwrap_or_else(|_| {
        if Command::new("llvm-ar-14").arg("--version").output().is_ok() {
            "llvm-ar-14".to_string()
        } else {
            "ar".to_string()
        }
    });

    let common = [
        "--target=wasm32-unknown-unknown",
        "-O3",
        "-DNDEBUG",
        "-DNO_SSE",
        "-ffreestanding",
        "-fno-builtin",
        "-Wall",
        "-Wno-unused-variable",
        "-Wno-unused-function",
        "-Wno-unknown-attributes",
        "-I",
        "vendor/include",
        "-I",
        "vendor",
        "-include",
        "ft_compat.h",
    ];

    let sources = [
        ("vendor/FastTree.c", "FastTree.o"),
        ("vendor/src/ft_compat.c", "ft_compat.o"),
        ("vendor/src/ft_reset.c", "ft_reset.o"),
    ];

    let mut objects = Vec::new();
    for (src, obj) in sources {
        let obj_path = out.join(obj);
        let obj_s = obj_path.to_str().unwrap();
        let mut args = common.to_vec();
        args.push("-c");
        args.push(src);
        args.push("-o");
        args.push(obj_s);
        run(&cc, &args);
        objects.push(obj_s.to_string());
    }

    let lib = out.join("libfasttree_c.a");
    let lib_s = lib.to_str().unwrap();
    let mut ar_args = vec!["rcs", lib_s];
    for o in &objects {
        ar_args.push(o);
    }
    run(&ar, &ar_args);

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=fasttree_c");
}
