//! Convert the inherited front end's panic diagnostics to ordinary errors.
pub fn catch<T>(
    action: impl FnOnce() -> Result<T, String> + std::panic::UnwindSafe,
) -> Result<T, String> {
    std::panic::catch_unwind(action).map_err(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "front end rejected input".into())
    })?
}
