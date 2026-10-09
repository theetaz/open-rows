// SPDX-License-Identifier: Apache-2.0
//! Installed-version detection and explicit override behavior.
use openrows_net::helper::{HelperError, tools};
use std::path::{Path, PathBuf};
#[tokio::test]
async fn detects_versions_and_never_falls_back_from_broken_override()
-> Result<(), Box<dyn std::error::Error>> {
    let fake = PathBuf::from(env!("CARGO_BIN_EXE_fake-cloud-hop"));
    for (_, tool) in tools::TOOLS {
        let found = tools::discover(tool, Some(&fake), &[]).await?;
        assert_eq!(found.path, fake);
        assert_eq!(found.version, "fake-cloud-hop 1.0.0");
    }
    let missing = tools::discover("kubectl", Some(Path::new("/not-an-installed-tool")), &[])
        .await
        .err();
    assert!(matches!(missing, Some(HelperError::NotFound { .. })));
    Ok(())
}
