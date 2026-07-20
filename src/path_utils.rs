use std::path::PathBuf;

#[cfg(target_os = "windows")]
pub(crate) fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    let bytes = path.as_os_str().as_encoded_bytes();
    let normalized = if let Some(suffix) = bytes
        .strip_prefix(br"\\?\UNC\")
        .or_else(|| bytes.strip_prefix(br"\\.\UNC\"))
    {
        [br"\\".as_slice(), suffix].concat()
    } else if let Some(suffix) = bytes
        .strip_prefix(br"\\?\")
        .or_else(|| bytes.strip_prefix(br"\\.\"))
        && suffix.get(1) == Some(&b':')
    {
        suffix.to_vec()
    } else {
        return path;
    };

    // SAFETY: the prefix manipulation preserves the platform path encoding returned by
    // `OsStr::as_encoded_bytes` and only removes ASCII bytes from a canonical Windows path.
    unsafe { PathBuf::from(std::ffi::OsStr::from_encoded_bytes_unchecked(&normalized)) }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    path
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn normalizes_windows_canonical_paths_for_shared_path_keys() {
        assert_eq!(
            normalize_canonical_path(PathBuf::from(r"\\?\C:\repo\src")),
            PathBuf::from(r"C:\repo\src")
        );
        assert_eq!(
            normalize_canonical_path(PathBuf::from(r"\\?\UNC\server\share\src")),
            PathBuf::from(r"\\server\share\src")
        );
    }
}
