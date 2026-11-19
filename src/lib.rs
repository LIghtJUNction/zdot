//! Allocation-free dotfile profile parsing.
//!
//! A profile is UTF-8, with one source and one destination separated by a tab.
//! Both paths are relative. The executable resolves paths through directory file
//! descriptors and refuses symlink parents; this library only validates syntax.
#![no_std]
#![forbid(unsafe_code)]

/// Maximum profile size accepted by the executable, in bytes.
pub const MAX_PROFILE: usize = 65_536;
/// Maximum mappings in one profile.
pub const MAX_ENTRIES: usize = 128;
/// Maximum path size, including the terminating NUL used by Linux.
pub const PATH_CAP: usize = 4096;

/// One borrowed, validated mapping.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Entry<'a> {
    /// Source path relative to the repository.
    pub source: &'a str,
    /// Destination path relative to the target directory.
    pub destination: &'a str,
}

/// A bounded table of borrowed entries.
#[derive(Debug)]
pub struct Profile<'a> {
    entries: [Entry<'a>; MAX_ENTRIES],
    len: usize,
}

impl<'a> Profile<'a> {
    /// Parse a complete profile before any filesystem mutation.
    pub fn parse(text: &'a str) -> Result<Self, &'static str> {
        if text.len() > MAX_PROFILE {
            return Err("profile exceeds 65536 bytes");
        }
        let mut result = Self {
            entries: [Entry {
                source: "",
                destination: "",
            }; MAX_ENTRIES],
            len: 0,
        };
        for raw in text.lines() {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let (source, destination) = line
                .split_once('\t')
                .ok_or("expected source<TAB>destination")?;
            validate_relative(source)?;
            validate_relative(destination)?;
            if destination
                .split('/')
                .next()
                .is_some_and(|x| x.starts_with(".zdot"))
            {
                return Err("destination uses reserved .zdot metadata prefix");
            }
            for entry in result.entries() {
                if overlaps(entry.destination, destination) {
                    return Err("duplicate or overlapping destinations");
                }
            }
            if result.len == MAX_ENTRIES {
                return Err("profile exceeds 128 mappings");
            }
            result.entries[result.len] = Entry {
                source,
                destination,
            };
            result.len += 1;
        }
        Ok(result)
    }

    /// Mappings in their original order.
    pub fn entries(&self) -> &[Entry<'a>] {
        &self.entries[..self.len]
    }
}

/// Reject ambiguous paths, absolute paths, traversal and control characters.
pub fn validate_relative(path: &str) -> Result<(), &'static str> {
    if path.is_empty() || path.len() >= PATH_CAP || path.starts_with('/') {
        return Err("paths must be nonempty and relative");
    }
    if path.bytes().any(|b| b < 32 || b == 127) {
        return Err("control characters are not supported in paths");
    }
    if path
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("empty, dot and parent components are forbidden");
    }
    if path.split('/').count() > 64 {
        return Err("path exceeds 64 components");
    }
    Ok(())
}

fn overlaps(a: &str, b: &str) -> bool {
    a == b
        || a.strip_prefix(b).is_some_and(|tail| tail.starts_with('/'))
        || b.strip_prefix(a).is_some_and(|tail| tail.starts_with('/'))
}

/// Stable namespace key for a repository and profile; owner metadata is also
/// checked, so a hash collision never authorizes another deployment.
pub fn namespace(root: &str, profile: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in root.bytes().chain([0]).chain(profile.bytes()) {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spaces_unicode_and_comments() {
        let profile = Profile::parse("# comment\r\nfiles/hello world\t.config/你好\r\n").unwrap();
        assert_eq!(
            profile.entries(),
            &[Entry {
                source: "files/hello world",
                destination: ".config/你好"
            }]
        );
    }

    #[test]
    fn rejects_escape_ambiguous_and_reserved_names() {
        for path in [
            "", "/", "../x", "a/../b", "./a", "a//b", "a/", "a\tb", "a\0b",
        ] {
            assert!(validate_relative(path).is_err(), "{path:?}");
        }
        assert!(Profile::parse("a\t.zdot-state/x").is_err());
    }

    #[test]
    fn rejects_overlapping_destinations_in_either_order() {
        for text in ["x\ta\ny\ta/b", "x\ta/b\ny\ta", "x\ta\ny\ta"] {
            assert!(Profile::parse(text).is_err());
        }
        assert!(Profile::parse("x\ta\ny\tab").is_ok());
    }

    #[test]
    fn namespaces_include_profile_and_repository() {
        assert_ne!(namespace("/a", "one"), namespace("/a", "two"));
        assert_ne!(namespace("/a", "one"), namespace("/b", "one"));
    }
}
