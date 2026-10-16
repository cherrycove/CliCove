use std::ffi::OsStr;
use std::process::Command;

/// Background CLI queries use pipes instead of a separate console window.
/// Interactive commands continue to use the embedded terminal's PTY.
pub(crate) fn background_command(program: impl AsRef<OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    command
}

#[cfg(all(test, windows))]
mod tests {
    use super::background_command;

    const CONSOLE_CHECK: &str = r#"Add-Type -Name ConsoleProbe -Namespace CliCove -MemberDefinition '[DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow();'; [CliCove.ConsoleProbe]::GetConsoleWindow().ToInt64(); [Console]::Error.WriteLine('stderr-ok'); exit 7"#;

    fn assert_no_console(output: std::process::Output) {
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "stderr-ok");
    }

    #[test]
    fn background_executable_has_no_console_and_preserves_output() {
        let output = background_command("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                CONSOLE_CHECK,
            ])
            .output()
            .unwrap();
        assert_no_console(output);
    }

    #[test]
    fn background_cmd_shim_has_no_console_and_preserves_output() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("console check.ps1");
        let shim = temp.path().join("cli shim.cmd");
        std::fs::write(&script, CONSOLE_CHECK).unwrap();
        std::fs::write(
            &shim,
            "@echo off\r\npowershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"%~dp0console check.ps1\"\r\nexit /b %errorlevel%\r\n",
        )
        .unwrap();
        let output = background_command(&shim).output().unwrap();
        assert_no_console(output);
    }
}
