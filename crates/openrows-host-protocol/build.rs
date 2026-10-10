// SPDX-License-Identifier: Apache-2.0
//! Lockstep build identity includes the lockfile and the host boundary sources.
use std::{fs, path::Path};

fn hash_tree(path: &Path, hash: &mut u64) -> std::io::Result<()> {
    if path.is_dir() {
        let mut paths = fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        for child in paths {
            hash_tree(&child, hash)?;
        }
    } else if path.extension().is_some_and(|extension| extension == "rs") {
        hash_bytes(&fs::read(path)?, hash);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    Ok(())
}

fn hash_bytes(bytes: &[u8], hash: &mut u64) {
    for byte in bytes {
        *hash = (*hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
    }
}

fn main() -> std::io::Result<()> {
    let root = Path::new("../..");
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for file in ["Cargo.lock", "Cargo.toml"] {
        hash_bytes(&fs::read(root.join(file))?, &mut hash);
        println!("cargo:rerun-if-changed=../../{file}");
    }
    for name in [
        "openrows-host-protocol",
        "openrows-host-client",
        "openrows-driver-host",
        "openrows-driver-api",
        "openrows-driver-sqlite",
    ] {
        let path = root.join("crates").join(name);
        let manifest = path.join("Cargo.toml");
        hash_bytes(&fs::read(&manifest)?, &mut hash);
        println!("cargo:rerun-if-changed={}", manifest.display());
        hash_tree(&path.join("src"), &mut hash)?;
    }
    println!("cargo:rustc-env=OPENROWS_HOST_BUILD={hash:016x}");
    Ok(())
}
