use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use std::{fmt, str::FromStr};

const MAX_KEY_BYTES: usize = 128;

///
/// `StableKey`
///
/// Canonical durable logical allocation identity.
///
/// A stable key names the logical store, not the current storage backend or
/// allocation ID. Once committed, the key is permanently bound to its
/// allocation; changing the key declares a new logical store.
///
/// Construction and deserialization enforce the same bounded canonical grammar.
/// A checked key establishes identity syntax, not allocation or open authority.
///

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StableKey(String);

impl<'de> Deserialize<'de> for StableKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Retain the serialized newtype shape before checking its owned text.
        #[derive(Deserialize)]
        #[serde(rename = "StableKey")]
        struct Text(String);

        let Text(value) = Text::deserialize(deserializer)?;
        validate(&value).map_err(D::Error::custom)?;
        Ok(Self(value))
    }
}

impl StableKey {
    /// Parse and validate a canonical stable key string.
    ///
    /// Keys are bounded lowercase ASCII dot-separated names ending in a
    /// nonzero `.vN` suffix.
    ///
    /// # Errors
    /// Returns an error when the name exceeds the length limit or violates stable-key syntax.
    pub fn parse(value: impl AsRef<str>) -> Result<Self, StableKeyError> {
        let value = value.as_ref();
        validate(value)?;
        Ok(Self(value.to_string()))
    }

    /// Borrow the canonical stable-key string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the key and return the canonical stable-key string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for StableKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for StableKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for StableKey {
    type Err = StableKeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

///
/// `StableKeyError`
///
/// Stable-key grammar validation failure.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
#[error("stable key '{stable_key}' is invalid: {reason}")]
pub struct StableKeyError {
    /// Rejected stable-key string.
    pub stable_key: String,
    /// Stable-key grammar failure.
    pub reason: &'static str,
}

pub fn validate(stable_key: &str) -> Result<(), StableKeyError> {
    if stable_key.is_empty() {
        return invalid(stable_key, "must not be empty");
    }
    if stable_key.len() > MAX_KEY_BYTES {
        return invalid(stable_key, "must be at most 128 bytes");
    }
    // Classify once, retaining the established diagnostic precedence even
    // when a key contains several different invalid characters.
    let mut invalid_ascii = false;
    let mut uppercase = false;
    let mut whitespace = false;
    let mut separator = false;
    for byte in stable_key.bytes() {
        invalid_ascii |= !byte.is_ascii();
        uppercase |= byte.is_ascii_uppercase();
        whitespace |= matches!(byte, b'\t'..=b'\r' | b' ');
        separator |= matches!(byte, b'/' | b'-');
    }
    if invalid_ascii {
        return invalid(stable_key, "must be ASCII");
    }
    if uppercase {
        return invalid(stable_key, "must be lowercase");
    }
    if whitespace {
        return invalid(stable_key, "must not contain whitespace");
    }
    if separator {
        return invalid(stable_key, "must not contain slashes or hyphens");
    }
    if stable_key.starts_with('.') || stable_key.ends_with('.') {
        return invalid(stable_key, "must not start or end with a dot");
    }

    let Some(version_index) = stable_key
        .as_bytes()
        .windows(2)
        .rposition(|pair| pair == b".v")
    else {
        return invalid(stable_key, "must end with .vN");
    };
    let version = &stable_key[version_index + 2..];
    if version.is_empty()
        || version.starts_with('0')
        || !version.bytes().all(|byte| byte.is_ascii_digit())
    {
        return invalid(stable_key, "version suffix must be nonzero .vN");
    }

    let prefix = &stable_key[..version_index];
    if prefix.is_empty() {
        return invalid(
            stable_key,
            "must contain at least one segment before version",
        );
    }

    for segment in prefix.as_bytes().split(|byte| *byte == b'.') {
        if let Err(reason) = validate_segment(segment) {
            return invalid(stable_key, reason);
        }
    }

    Ok(())
}

// A namespace plus its synthetic `v1` suffix must fit the same key grammar.
// Borrow the prefix rather than allocating a temporary key just to discard it.
pub fn is_key_prefix(value: &str) -> bool {
    let Some(prefix) = value.strip_suffix('.') else {
        return false;
    };
    value.len() <= MAX_KEY_BYTES - "v1".len()
        && prefix
            .as_bytes()
            .split(|byte| *byte == b'.')
            .all(|segment| validate_segment(segment).is_ok())
}

fn validate_segment(segment: &[u8]) -> Result<(), &'static str> {
    let mut bytes = segment.iter().copied();
    let Some(first) = bytes.next() else {
        return Err("must not contain empty segments");
    };
    if !first.is_ascii_lowercase() {
        return Err("segments must start with a lowercase letter");
    }
    if !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_') {
        return Err("segments may contain only lowercase letters, digits, and underscores");
    }
    Ok(())
}

fn invalid<T>(stable_key: &str, reason: &'static str) -> Result<T, StableKeyError> {
    Err(StableKeyError {
        stable_key: stable_key.to_string(),
        reason,
    })
}

#[cfg(test)]
mod tests;
