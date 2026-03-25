use std::process::Command;

/// Activate the application window/tab that owns the given PID.
pub fn focus_terminal_for_pid(pid: u32) -> bool {
    let app_path = match find_owner_app(pid) {
        Some(p) => p,
        None => return false,
    };

    let tty = get_tty(pid);

    // Try app-specific tab focusing first, fall back to generic open
    if let Some(tty) = &tty {
        if app_path.contains("iTerm") && focus_iterm_tab(tty) {
            return true;
        }
        if app_path.contains("Terminal.app") && focus_terminal_app_tab(tty) {
            return true;
        }
    }

    // For VS Code, Warp, etc: just activate the app
    Command::new("open")
        .args(["-a", &app_path])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn get_tty(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "tty=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tty.is_empty() || tty == "??" {
        None
    } else {
        Some(format!("/dev/{}", tty))
    }
}

fn focus_iterm_tab(tty: &str) -> bool {
    let script = format!(
        r#"tell application "iTerm2"
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                if tty of s is "{tty}" then
                    select t
                    select s
                    set index of w to 1
                    activate
                    return true
                end if
            end repeat
        end repeat
    end repeat
end tell
return false"#
    );
    run_osascript(&script)
}

fn focus_terminal_app_tab(tty: &str) -> bool {
    let script = format!(
        r#"tell application "Terminal"
    repeat with w in windows
        repeat with t in tabs of w
            if tty of t is "{tty}" then
                set selected tab of w to t
                set index of w to 1
                activate
                return true
            end if
        end repeat
    end repeat
end tell
return false"#
    );
    run_osascript(&script)
}

fn run_osascript(script: &str) -> bool {
    Command::new("osascript")
        .args(["-e", script])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

fn find_owner_app(pid: u32) -> Option<String> {
    let mut current = pid;

    for _ in 0..20 {
        let cmd = get_command(current)?;
        if let Some(app) = extract_app_path(&cmd) {
            return Some(app);
        }

        let ppid = get_ppid(current)?;
        if ppid <= 1 {
            // Reached launchd without finding a .app in the process tree.
            // Fall back to checking which app owns the TTY (handles Terminal.app
            // whose child processes are login/shell with no .app in the path).
            return get_tty(pid).and_then(|tty| find_app_by_tty(&tty));
        }
        current = ppid;
    }

    None
}

/// Use lsof to find which .app owns the given TTY device.
fn find_app_by_tty(tty: &str) -> Option<String> {
    let output = Command::new("lsof")
        .args(["-t", tty])
        .output()
        .ok()?;
    let pids_str = String::from_utf8_lossy(&output.stdout);
    for line in pids_str.lines() {
        if let Ok(pid) = line.trim().parse::<u32>() {
            if let Some(cmd) = get_command(pid) {
                if let Some(app) = extract_app_path(&cmd) {
                    return Some(app);
                }
            }
        }
    }
    None
}

fn get_ppid(pid: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()
}

fn get_command(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let cmd = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if cmd.is_empty() { None } else { Some(cmd) }
}

fn extract_app_path(cmd: &str) -> Option<String> {
    let idx = cmd.find(".app")?;
    Some(cmd[..idx + 4].to_string())
}
