// SPDX-License-Identifier: Apache-2.0
//! Text decoding while streaming: any WHATWG encoding becomes UTF-8 for the
//! parsers, a byte-order mark is honoured and removed, and the bytes read
//! from the file are counted for progress.
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use encoding_rs::{CoderResult, Decoder, Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1252};

use crate::ImportError;

/// Raw bytes read from the file per refill.
const INPUT: usize = 64 * 1024;
/// Decoded UTF-8 buffered per refill; worst case is three bytes per input byte.
const OUTPUT: usize = INPUT * 3 + 16;

/// The encoding named by `label`.
///
/// # Errors
/// [`ImportError::UnknownEncoding`].
pub(crate) fn encoding(label: &str) -> Result<&'static Encoding, ImportError> {
    Encoding::for_label(label.trim().as_bytes())
        .ok_or_else(|| ImportError::UnknownEncoding(label.to_owned()))
}

/// The most likely encoding of a file starting with `head`: its byte-order
/// mark, UTF-16 when every other byte is NUL, UTF-8 when the bytes are
/// valid UTF-8 (a sequence cut at the end is allowed), else Windows-1252.
#[must_use]
pub(crate) fn sniff_encoding(head: &[u8]) -> &'static Encoding {
    if let Some((encoding, _)) = Encoding::for_bom(head) {
        return encoding;
    }
    let pairs = head.len() / 2;
    if pairs >= 4 {
        let zero_odd = head.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
        let zero_even = head.iter().step_by(2).filter(|&&b| b == 0).count();
        if zero_odd * 10 >= pairs * 9 && zero_even == 0 {
            return UTF_16LE;
        }
        if zero_even * 10 >= pairs * 9 && zero_odd == 0 {
            return UTF_16BE;
        }
    }
    match std::str::from_utf8(head) {
        Ok(_) => UTF_8,
        Err(error) if error.error_len().is_none() => UTF_8,
        Err(_) => WINDOWS_1252,
    }
}

/// Counts the raw bytes read through it, shared with a progress reporter.
pub(crate) struct Counting<R> {
    inner: R,
    read: Arc<AtomicU64>,
}

impl<R> Counting<R> {
    pub(crate) fn new(inner: R, read: Arc<AtomicU64>) -> Self {
        Self { inner, read }
    }
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buf)?;
        self.read.fetch_add(count as u64, Ordering::Relaxed);
        Ok(count)
    }
}

/// A reader yielding the UTF-8 text of `inner`, decoded from `encoding`.
/// Malformed sequences become U+FFFD instead of failing the import.
pub(crate) struct Decoded<R> {
    inner: R,
    decoder: Decoder,
    input: Box<[u8]>,
    /// Unconsumed input is `input[start..end]`.
    start: usize,
    end: usize,
    eof: bool,
    output: Box<[u8]>,
    /// Undelivered output is `output[out_start..out_end]`.
    out_start: usize,
    out_end: usize,
    finished: bool,
}

impl<R: Read> Decoded<R> {
    pub(crate) fn new(inner: R, encoding: &'static Encoding) -> Self {
        Self {
            inner,
            // BOM sniffing: a byte-order mark wins over the chosen encoding.
            decoder: encoding.new_decoder(),
            input: vec![0; INPUT].into_boxed_slice(),
            start: 0,
            end: 0,
            eof: false,
            output: vec![0; OUTPUT].into_boxed_slice(),
            out_start: 0,
            out_end: 0,
            finished: false,
        }
    }

    /// Decode the next piece of input into the empty output buffer.
    fn refill(&mut self) -> io::Result<()> {
        if self.start == self.end && !self.eof {
            self.start = 0;
            self.end = self.inner.read(&mut self.input)?;
            self.eof = self.end == 0;
        }
        let (result, read, written, _) = self.decoder.decode_to_utf8(
            &self.input[self.start..self.end],
            &mut self.output,
            self.eof,
        );
        self.start += read;
        self.out_start = 0;
        self.out_end = written;
        if self.eof && result == CoderResult::InputEmpty {
            self.finished = true;
        }
        Ok(())
    }
}

impl<R: Read> Read for Decoded<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.out_start < self.out_end {
                let count = buf.len().min(self.out_end - self.out_start);
                buf[..count].copy_from_slice(&self.output[self.out_start..self.out_start + count]);
                self.out_start += count;
                return Ok(count);
            }
            if self.finished {
                return Ok(0);
            }
            self.refill()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8], label: &str) -> Result<String, ImportError> {
        let mut text = String::new();
        Decoded::new(bytes, encoding(label)?).read_to_string(&mut text)?;
        Ok(text)
    }

    #[test]
    fn encodings_decode_to_utf8_and_drop_the_bom() -> Result<(), ImportError> {
        assert_eq!(decode(b"\xEF\xBB\xBFa,b", "utf-8")?, "a,b");
        assert_eq!(decode(b"caf\xE9", "windows-1252")?, "café");
        assert_eq!(decode(b"\xFF\xFEa\0,\0b\0", "utf-8")?, "a,b");
        assert_eq!(decode(b"a\0\xE9\0", "utf-16le")?, "aé");
        assert_eq!(decode(b"bad \xFF end", "utf-8")?, "bad \u{FFFD} end");
        assert!(matches!(
            encoding("klingon"),
            Err(ImportError::UnknownEncoding(_))
        ));
        Ok(())
    }

    #[test]
    fn sniffing_prefers_bom_then_utf16_then_utf8() {
        assert_eq!(sniff_encoding(b"\xEF\xBB\xBFx"), UTF_8);
        assert_eq!(sniff_encoding(b"\xFE\xFF\0x"), UTF_16BE);
        assert_eq!(sniff_encoding(b"i\0d\0,\0n\0a\0m\0e\0"), UTF_16LE);
        assert_eq!(sniff_encoding(b"\0i\0d\0,\0n\0a\0m\0e"), UTF_16BE);
        assert_eq!(sniff_encoding("ünï 😀".as_bytes()), UTF_8);
        // A multi-byte character cut by the sample boundary is still UTF-8.
        assert_eq!(sniff_encoding(&"é".as_bytes()[..1]), UTF_8);
        assert_eq!(sniff_encoding(b"caf\xE9 au lait"), WINDOWS_1252);
    }

    #[test]
    fn large_inputs_stream_through_small_buffers() -> Result<(), ImportError> {
        let line = "é,ü,ß\n".repeat(50_000);
        let latin: Vec<u8> = line
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
            .collect();
        let read = Arc::new(AtomicU64::new(0));
        let counting = Counting::new(latin.as_slice(), Arc::clone(&read));
        let mut text = String::new();
        Decoded::new(counting, WINDOWS_1252).read_to_string(&mut text)?;
        assert_eq!(text, line);
        assert_eq!(read.load(Ordering::Relaxed), latin.len() as u64);
        Ok(())
    }
}
