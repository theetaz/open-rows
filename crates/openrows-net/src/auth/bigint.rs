// SPDX-License-Identifier: Apache-2.0
//! Minimal unsigned big-integer helpers to derive RSA CRT exponents
//! (`dP = d mod (p-1)`, `dQ = d mod (q-1)`), which OpenSSH key files omit
//! but ring requires.
//!
//! Runs once, locally, at key load. It is not constant-time, which is
//! acceptable because no remote party can time key loading; all signing is
//! done by ring's constant-time code.

/// `value - 1` for a big-endian integer; `None` for zero.
pub(crate) fn minus_one(value: &[u8]) -> Option<Vec<u8>> {
    let mut limbs = to_limbs(value);
    for limb in &mut limbs {
        let (next, borrow) = limb.overflowing_sub(1);
        *limb = next;
        if !borrow {
            return Some(to_be_bytes(&limbs));
        }
    }
    None
}

/// `value mod modulus` for big-endian integers; `None` if `modulus` is zero.
pub(crate) fn remainder(value: &[u8], modulus: &[u8]) -> Option<Vec<u8>> {
    let mut divisor = to_limbs(modulus);
    if divisor.iter().all(|&limb| limb == 0) {
        return None;
    }
    divisor.push(0);
    let mut rest = vec![0_u32; divisor.len()];
    for byte in value {
        for shift in (0..8).rev() {
            shift_left_with(&mut rest, u32::from((byte >> shift) & 1));
            if !is_less(&rest, &divisor) {
                subtract(&mut rest, &divisor);
            }
        }
    }
    Some(to_be_bytes(&rest))
}

/// Little-endian `u32` limbs.
fn to_limbs(be: &[u8]) -> Vec<u32> {
    be.rchunks(4)
        .map(|chunk| {
            chunk
                .iter()
                .fold(0_u32, |acc, &byte| (acc << 8) | u32::from(byte))
        })
        .collect()
}

/// Big-endian bytes without leading zeros (one zero byte for zero).
fn to_be_bytes(limbs: &[u32]) -> Vec<u8> {
    let bytes: Vec<u8> = limbs
        .iter()
        .rev()
        .flat_map(|limb| limb.to_be_bytes())
        .collect();
    let first = bytes
        .iter()
        .position(|&byte| byte != 0)
        .unwrap_or(bytes.len().saturating_sub(1));
    bytes.get(first..).map(<[u8]>::to_vec).unwrap_or_default()
}

fn shift_left_with(limbs: &mut [u32], low_bit: u32) {
    let mut carry = low_bit;
    for limb in limbs {
        let next_carry = *limb >> 31;
        *limb = (*limb << 1) | carry;
        carry = next_carry;
    }
}

fn is_less(left: &[u32], right: &[u32]) -> bool {
    left.iter().rev().cmp(right.iter().rev()).is_lt()
}

fn subtract(left: &mut [u32], right: &[u32]) {
    let mut borrow = false;
    for (limb, &other) in left.iter_mut().zip(right) {
        let (value, first) = limb.overflowing_sub(other);
        let (value, second) = value.overflowing_sub(u32::from(borrow));
        *limb = value;
        borrow = first || second;
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{minus_one, remainder};

    fn be(value: u128) -> Vec<u8> {
        let bytes = value.to_be_bytes();
        let first = bytes.iter().position(|&byte| byte != 0).unwrap_or(15);
        bytes[first..].to_vec()
    }

    #[test]
    fn edge_cases() {
        assert_eq!(minus_one(&[0]), None);
        assert_eq!(minus_one(&[1, 0]), Some(vec![255]));
        assert_eq!(remainder(&[5], &[0]), None);
        assert_eq!(remainder(&[], &[7]), Some(vec![0]));
    }

    proptest! {
        #[test]
        fn matches_native_arithmetic(value in any::<u128>(), modulus in 1..=u128::MAX) {
            prop_assert_eq!(remainder(&be(value), &be(modulus)), Some(be(value % modulus)));
            prop_assert_eq!(minus_one(&be(modulus)), Some(be(modulus - 1)));
        }
    }
}
