// SPDX-License-Identifier: Apache-2.0
//! Explicit IPC generator; never rewrites bindings during app startup.
fn main() -> Result<(), specta_typescript::Error> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    openrows_app::export_bindings(&root.join("../../../packages/ipc/src/generated/bindings.ts"))
}
