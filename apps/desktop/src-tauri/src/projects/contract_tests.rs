// SPDX-License-Identifier: Apache-2.0
//! Project sharing and snippet commands must be available to the main window.
#[test]
fn project_sharing_and_snippet_commands_are_registered_and_allowed()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("build.rs"))?;
    let capability: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("capabilities/main.json"),
    )?)?;
    let permissions = capability["permissions"]
        .as_array()
        .ok_or("Missing permissions")?;
    for command in [
        "project_share_connections",
        "project_sql_items",
        "project_create_snippet",
    ] {
        assert!(manifest.contains(&format!("\"{command}\"")));
        let permission = format!("allow-{}", command.replace('_', "-"));
        assert!(
            permissions
                .iter()
                .any(|item| item.as_str() == Some(&permission))
        );
    }
    Ok(())
}
