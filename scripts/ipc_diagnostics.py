# SPDX-License-Identifier: Apache-2.0
"""Bounded diagnostics for an IPC launch that did not deliver a usable report."""


def print_diagnostics(report, stderr):
    """Print last lines only; both files live in the launch's temporary directory."""
    for label, path in (("IPC progress", report.with_suffix(".progress")),
                        ("App stderr", stderr)):
        if path.exists():
            with path.open("rb") as stream:
                stream.seek(0, 2)
                stream.seek(max(0, stream.tell() - 8192))
                tail = stream.read().decode("utf-8", errors="replace").splitlines()[-30:]
        else:
            tail = []
        print(f"{label} tail:\n" + ("\n".join(tail) if tail else "(empty)"))
