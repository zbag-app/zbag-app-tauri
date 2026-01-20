use std::ops::Deref;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A `String` that is treated as sensitive and is zeroized on drop.
///
/// This helps limit secret retention on the Rust side (e.g., after IPC serialization).
/// It does **not** guarantee zeroization across FFI or JS runtimes.
///
/// # Cloning
///
/// `SensitiveString` implements `Clone` for ergonomics when passing IPC payloads, but cloning
/// duplicates the sensitive value in memory. Prefer to avoid cloning where possible.
///
/// # Comparisons
///
/// `SensitiveString` derives `PartialEq`/`Eq`, which is a standard string comparison and is not
/// constant-time. Avoid using it for authentication or other side-channel-sensitive comparisons.
///
/// # Display
///
/// `SensitiveString` intentionally does not implement `Display`, to reduce the chance of
/// accidentally printing secrets.
///
/// ```compile_fail
/// use zstash_core::sensitive::SensitiveString;
/// let s = SensitiveString::from("secret");
/// println!("{s}");
/// ```
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
#[must_use]
pub struct SensitiveString(String);

impl SensitiveString {
    /// Construct a `SensitiveString` from an owned `String`.
    ///
    /// This moves the string into the wrapper without copying. Note that any *previous* clones of
    /// the string data may still exist elsewhere in memory.
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Trim leading and trailing whitespace in-place, zeroizing the removed bytes.
    ///
    /// This avoids allocating a second copy of the sensitive string (e.g., during CLI argument
    /// parsing) while still normalizing user input.
    pub fn trim_in_place(&mut self) {
        let trimmed = self.0.trim();
        if trimmed.len() == self.0.len() {
            return;
        }

        // `trim()` returns a subslice of the original string, but computing the byte range via
        // pointer subtraction is more subtle than we need here. Instead, compute `start` via
        // length differences and derive `end` from the trimmed length.
        let start = self.0.len() - self.0.trim_start().len();
        let new_len = trimmed.len();
        let end = start + new_len;

        // These bounds ensure the new string remains valid UTF-8 after the in-place move.
        assert!(self.0.is_char_boundary(start));
        assert!(self.0.is_char_boundary(end));

        // Safety:
        // - `trim()` returns a subslice of `self.0` that is valid UTF-8 and aligned to char
        //   boundaries.
        // - We copy exactly those bytes to the start of the buffer, zeroize the remainder of the
        //   original length, and then truncate to the new length.
        // - The resulting `String` stays valid UTF-8.
        unsafe {
            let bytes = self.0.as_mut_vec();
            let old_len = bytes.len();
            bytes.copy_within(start..end, 0);
            bytes[new_len..old_len].zeroize();
            bytes.truncate(new_len);
        }
    }
}

impl Deref for SensitiveString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl AsRef<str> for SensitiveString {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for SensitiveString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SensitiveString {
    /// Note: this copies the string data. The original `&str` remains in memory and must be
    /// zeroized separately if needed.
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::str::FromStr for SensitiveString {
    // Infallible: we don't validate/normalize input here; we just copy it into a new owned
    // `SensitiveString`. This is mainly for CLI/IPC parsing ergonomics.
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(value))
    }
}

impl std::fmt::Debug for SensitiveString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::SensitiveString;
    use zeroize::ZeroizeOnDrop;

    #[test]
    fn debug_is_redacted() {
        let s = SensitiveString::from("secret");
        assert_eq!(format!("{s:?}"), "[REDACTED]");
    }

    #[test]
    fn implements_zeroize_on_drop() {
        fn assert_impl<T: ZeroizeOnDrop>() {}
        assert_impl::<SensitiveString>();
    }

    #[test]
    fn deref_and_as_ref_work() {
        let s = SensitiveString::from("secret");
        assert_eq!(s.as_ref(), "secret");
        assert_eq!(&*s, "secret");
    }

    #[test]
    fn serde_roundtrip_is_transparent() {
        let s = SensitiveString::from("secret");
        let json = serde_json::to_string(&s).expect("serialize");
        assert_eq!(json, "\"secret\"");

        let decoded: SensitiveString = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.as_ref(), "secret");
    }

    #[test]
    fn trim_in_place_removes_whitespace() {
        let mut s = SensitiveString::from("  secret \n");
        s.trim_in_place();
        assert_eq!(s.as_ref(), "secret");
    }

    #[test]
    fn trim_in_place_all_whitespace_is_empty() {
        let mut s = SensitiveString::from(" \n\t");
        s.trim_in_place();
        assert!(s.as_ref().is_empty());
    }

    #[test]
    fn trim_in_place_empty_is_empty() {
        let mut s = SensitiveString::from("");
        s.trim_in_place();
        assert!(s.as_ref().is_empty());
    }

    #[test]
    fn trim_in_place_leading_whitespace_only() {
        let mut s = SensitiveString::from(" \n\tsecret");
        s.trim_in_place();
        assert_eq!(s.as_ref(), "secret");
    }

    #[test]
    fn trim_in_place_trailing_whitespace_only() {
        let mut s = SensitiveString::from("secret \n\t");
        s.trim_in_place();
        assert_eq!(s.as_ref(), "secret");
    }
}
