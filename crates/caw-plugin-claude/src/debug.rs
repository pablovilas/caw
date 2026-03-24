use crate::process;
use sysinfo::System;

pub fn debug_processes() {
    let procs = process::get_claude_processes();
    eprintln!("=== Running Claude processes ===");
    for p in &procs {
        eprintln!("  PID={} cwd={:?}", p.pid, p.cwd);
    }

    eprintln!("\n=== Discovered Claude instances ===");
    let instances = process::discover_claude_instances();
    for inst in &instances {
        let branch = inst.extra.get("git_branch").and_then(|v| v.as_str()).unwrap_or("-");
        eprintln!(
            "  id={} pid={:?} dir={} branch={}",
            inst.id,
            inst.pid,
            inst.working_dir.display(),
            branch
        );
    }

    eprintln!("\n=== Codex/OpenCode processes (sysinfo) ===");
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_string();
        if name == "codex" || name.starts_with("codex") || name == "opencode" || name.starts_with("opencode") {
            eprintln!("  PID={} name={}", pid.as_u32(), name);
        }
    }
}
