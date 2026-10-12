# SPDX-License-Identifier: Apache-2.0
"""Generate public release configuration; private credentials stay in the environment."""
import json
import os
import re
import sys


def release_config(env):
    """A signed update build requires both channels and a pinned public key."""
    key = env.get("TAURI_SIGNING_PUBLIC_KEY", "")
    repository = env.get("GITHUB_REPOSITORY", "")
    if not key or not re.fullmatch(r"[A-Za-z0-9][\w.-]*/[A-Za-z0-9][\w.-]*", repository):
        raise ValueError("TAURI_SIGNING_PUBLIC_KEY and GITHUB_REPOSITORY are required")
    base = f"https://github.com/{repository}/releases"
    feeds = {"stable": f"{base}/latest/download/stable.json",
             "beta": f"{base}/download/update-beta/beta.json", "publicKey": key}
    return {"bundle": {"createUpdaterArtifacts": True},
            "plugins": {"updater": {"pubkey": key, "endpoints": [feeds["stable"]]},
                        "openrowsUpdates": feeds}}


if __name__ == "__main__":
    json.dump(release_config(os.environ), sys.stdout)
