/// Aborts on a programming panic without printing potentially sensitive panic payloads.
pub(crate) fn install() {
    std::panic::set_hook(Box::new(|_| {
        tracing::error!(
            code = "PROCESS_PANIC",
            "A programming panic requires process termination"
        );
        std::process::abort();
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn panic_aborts_without_exposing_payload() {
        const CHILD: &str = "VELO_PANIC_POLICY_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            super::install();
            panic!("dummy-panic-secret");
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "panic_policy::tests::panic_aborts_without_exposing_payload",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("dummy-panic-secret"));
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(output.status.signal(), Some(6));
        }
    }
}
