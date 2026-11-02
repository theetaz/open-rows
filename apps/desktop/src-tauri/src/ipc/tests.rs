// SPDX-License-Identifier: Apache-2.0
//! Contract checks tying the bindings to the registry, manifest and capability.
#[test]
fn exported_commands_are_registered_and_allowed_in_the_main_window()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bindings =
        std::fs::read_to_string(root.join("../../../packages/ipc/src/generated/bindings.ts"))?;
    let manifest = std::fs::read_to_string(root.join("build.rs"))?;
    let capability: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("capabilities/main.json"),
    )?)?;
    let permissions = capability["permissions"]
        .as_array()
        .ok_or("Missing permissions")?;
    let mut checked = 0;
    for line in bindings
        .lines()
        .filter(|line| line.contains("__TAURI_INVOKE"))
    {
        let Some((_, rest)) = line.rsplit_once("(\"") else {
            continue;
        };
        let Some((command, _)) = rest.split_once('"') else {
            continue;
        };
        assert!(
            manifest.contains(&format!("\"{command}\"")),
            "Unregistered command: {command}"
        );
        let permission = format!("allow-{}", command.replace('_', "-"));
        assert!(
            permissions
                .iter()
                .any(|item| item.as_str() == Some(&permission)),
            "Missing permission: {permission}"
        );
        checked += 1;
    }
    assert!(checked >= 30, "No command contract was checked");
    Ok(())
}
#[test]
fn user_mcp_commands_are_registered_and_allowed() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("build.rs"))?;
    let capability: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        root.join("capabilities/main.json"),
    )?)?;
    let permissions = capability["permissions"]
        .as_array()
        .ok_or("Missing permissions")?;
    for command in [
        "mcp_servers_list",
        "mcp_servers_save",
        "mcp_server_secrets",
        "mcp_server_delete",
        "mcp_server_tools",
        "mcp_tool_confirm",
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
#[test]
fn bindings_are_fresh() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let temporary = root.join(format!(".bindings-check-{}.ts", std::process::id()));
    let generated: Result<String, Box<dyn std::error::Error>> = (|| {
        super::export_bindings(&temporary)?;
        std::fs::read_to_string(&temporary).map_err(Into::into)
    })();
    if temporary.exists() {
        std::fs::remove_file(&temporary)?;
    }
    let expected =
        std::fs::read_to_string(root.join("../../../packages/ipc/src/generated/bindings.ts"))?;
    let actual: String = generated?;
    assert_eq!(actual, expected, "IPC bindings are stale; run just gen-ipc");
    Ok(())
}
