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
        .map_err(|error| io_failure("native_header_read", &error))?;
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
        tracing::warn!(
            check = "native_format",
            expected = std::env::consts::OS,
            "driver host compatibility check failed"
        );
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
    header[..4] == *b"\x7fELF"
        && check("elf_class", u32::from(header[4]), 2)
        && check("elf_endianness", u32::from(header[5]), 1)
        && check("elf_version", u32::from(header[6]), 1)
        && check(
            "elf_machine",
            u32::from(u16::from_le_bytes([header[18], header[19]])),
            architecture(183, 62),
        )
}
fn check(name: &'static str, observed: u32, expected: u32) -> bool {
    if observed != expected {
        tracing::warn!(
            check = name,
            observed,
            expected,
            "driver host compatibility check failed"
        );
    }
    observed == expected
}
fn io_failure(check: &'static str, error: &std::io::Error) -> HostFailure {
    tracing::warn!(check, error = ?error.kind(), "driver host compatibility check failed");
    HostFailure::Incompatible
}
fn macho(binary: &mut File, header: &[u8; 64]) -> Result<bool, HostFailure> {
    let expected = architecture(0x0100_000c, 0x0100_0007);
    if header[..4] == [0xcf, 0xfa, 0xed, 0xfe] {
        return Ok(check(
            "macho_cpu",
            u32::from_le_bytes(
                header[4..8]
                    .try_into()
                    .map_err(|_| HostFailure::Incompatible)?,
            ),
            expected,
        ) && check(
            "macho_type",
            u32::from_le_bytes([header[12], header[13], header[14], header[15]]),
            2,
        ));
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
        tracing::warn!(
            check = "macho_fat_count",
            observed = count,
            min = 1,
            max = 16,
            "driver host compatibility check failed"
        );
        return Ok(false);
    }
    binary
        .seek(SeekFrom::Start(8))
        .map_err(|error| io_failure("macho_fat_seek", &error))?;
    let mut found = false;
    for _ in 0..count {
        let mut entry = [0; 20];
        binary
            .read_exact(&mut entry)
            .map_err(|error| io_failure("macho_fat_entry_read", &error))?;
        found |= u32::from_be_bytes(
            entry[..4]
                .try_into()
                .map_err(|_| HostFailure::Incompatible)?,
        ) == expected;
    }
    if !found {
        tracing::warn!(
            check = "macho_fat_cpu",
            expected,
            "driver host compatibility check failed"
        );
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
        tracing::warn!(
            check = "pe_header_offset",
            observed = offset,
            min = 64,
            max = 1024 * 1024,
            "driver host compatibility check failed"
        );
        return Ok(false);
    }
    binary
        .seek(SeekFrom::Start(u64::from(offset)))
        .map_err(|error| io_failure("pe_header_seek", &error))?;
    let mut signature = [0; 6];
    binary
        .read_exact(&mut signature)
        .map_err(|error| io_failure("pe_header_read", &error))?;
    Ok(signature[..4] == *b"PE\0\0"
        && check(
            "pe_machine",
            u32::from(u16::from_le_bytes([signature[4], signature[5]])),
            architecture(0xaa64, 0x8664),
        ))
}

#[cfg(test)]
mod tests;
