//! Desktop-independent entry point for Velo's native engine.

/// Returns the engine's build identity without executing network requests.
///
/// # Returns
/// The engine name and Cargo package version. No resources are acquired.
///
/// # Examples
/// ```
/// let (name, version) = velo_engine::identity();
/// assert_eq!(name, "velo-engine");
/// assert!(!version.is_empty());
/// ```
pub fn identity() -> (&'static str, &'static str) {
    ("velo-engine", env!("CARGO_PKG_VERSION"))
}
