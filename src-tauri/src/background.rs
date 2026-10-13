/// Run filesystem, process and blocking HTTP work without occupying the window
/// thread or an async executor worker. The original command result is preserved.
pub(crate) async fn run<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("后台任务未能完成：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn work_runs_on_another_thread_and_returns_its_value() {
        let caller = std::thread::current().id();
        let worker = tauri::async_runtime::block_on(run(|| Ok(std::thread::current().id())));
        assert_ne!(worker.unwrap(), caller);
    }

    #[test]
    fn operation_errors_are_preserved() {
        let result = tauri::async_runtime::block_on(run(|| Err::<(), _>("probe failed".into())));
        assert_eq!(result, Err("probe failed".into()));
    }
}
