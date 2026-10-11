// SPDX-License-Identifier: Apache-2.0
//! ELF rules run as pure header tests on macOS, without compiling a Linux target.
use super::*;

fn header() -> [u8; 64] {
    let mut header = [0; 64];
    header[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    let machine = if cfg!(target_arch = "aarch64") {
        183_u16
    } else {
        62
    };
    header[18..20].copy_from_slice(&machine.to_le_bytes());
    header
}

#[test]
fn elf_accepts_executable_and_pie_without_interpreter_checks() {
    for executable_type in [2_u16, 3] {
        let mut header = header();
        header[16..18].copy_from_slice(&executable_type.to_le_bytes());
        assert!(elf(&header));
    }
}

#[test]
fn elf_rejects_foreign_machine_class_endianness_and_version() {
    for (offset, value) in [(0, 0), (4, 1), (5, 2), (6, 0), (18, 0)] {
        let mut header = header();
        header[offset] = value;
        assert!(!elf(&header));
    }
}
