// SPDX-License-Identifier: Apache-2.0
//! Bounded native-header checks reject scripts, foreign formats and wrong architectures.
use crate::HostFailure;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

pub(crate) fn validate(binary: &mut File) -> Result<(), HostFailure> {
    let mut header = [0; 64];
    binary
        .read_exact(&mut header)
        .map_err(|_| HostFailure::Incompatible)?;
    let valid = if cfg!(target_os = "macos") {
        macho(binary, &header)?
    } else if cfg!(target_os = "linux") {
        elf(&header)
    } else if cfg!(windows) {
        pe(binary, &header)?
    } else {
        false
    };
    if valid {
        Ok(())
    } else {
        Err(HostFailure::Incompatible)
    }
}
fn architecture(arm: u32, x64: u32) -> u32 {
    if cfg!(target_arch = "aarch64") {
        arm
    } else if cfg!(target_arch = "x86_64") {
        x64
    } else {
        0
    }
}
fn elf(header: &[u8; 64]) -> bool {
    header[..7] == *b"\x7fELF\x02\x01\x01"
        && u32::from(u16::from_le_bytes([header[18], header[19]])) == architecture(183, 62)
}
fn macho(binary: &mut File, header: &[u8; 64]) -> Result<bool, HostFailure> {
    let expected = architecture(0x0100_000c, 0x0100_0007);
    if header[..4] == [0xcf, 0xfa, 0xed, 0xfe] {
        return Ok(u32::from_le_bytes(
            header[4..8]
                .try_into()
                .map_err(|_| HostFailure::Incompatible)?,
        ) == expected
            && header[12..16] == [2, 0, 0, 0]);
    }
    if header[..4] != [0xca, 0xfe, 0xba, 0xbe] {
        return Ok(false);
    }
    let count = u32::from_be_bytes(
        header[4..8]
            .try_into()
            .map_err(|_| HostFailure::Incompatible)?,
    );
    if count == 0 || count > 16 {
        return Ok(false);
    }
    binary
        .seek(SeekFrom::Start(8))
        .map_err(|_| HostFailure::Incompatible)?;
    let mut found = false;
    for _ in 0..count {
        let mut entry = [0; 20];
        binary
            .read_exact(&mut entry)
            .map_err(|_| HostFailure::Incompatible)?;
        found |= u32::from_be_bytes(
            entry[..4]
                .try_into()
                .map_err(|_| HostFailure::Incompatible)?,
        ) == expected;
    }
    Ok(found)
}
fn pe(binary: &mut File, header: &[u8; 64]) -> Result<bool, HostFailure> {
    if header[..2] != *b"MZ" {
        return Ok(false);
    }
    let offset = u32::from_le_bytes(
        header[60..64]
            .try_into()
            .map_err(|_| HostFailure::Incompatible)?,
    );
    if !(64..=1024 * 1024).contains(&offset) {
        return Ok(false);
    }
    binary
        .seek(SeekFrom::Start(u64::from(offset)))
        .map_err(|_| HostFailure::Incompatible)?;
    let mut signature = [0; 6];
    binary
        .read_exact(&mut signature)
        .map_err(|_| HostFailure::Incompatible)?;
    Ok(signature[..4] == *b"PE\0\0"
        && u32::from(u16::from_le_bytes([signature[4], signature[5]]))
            == architecture(0xaa64, 0x8664))
}
