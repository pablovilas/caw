use crate::process;
use caw_core::ProcessScanner;

pub fn debug_processes() {
    let mut scanner = ProcessScanner::new();

    let procs = scanner.scan(&["claude"]);
    eprintln!("=== Running Claude processes ===");
    for p in &procs {
        eprintln!("  PID={} cwd={:?}", p.pid, p.cwd);
    }

    eprintln!("\n=== Discovered Claude instances ===");
    let instances = process::discover_claude_instances(procs);
    for inst in &instances {
        let branch = inst
            .extra
            .get("git_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        eprintln!(
            "  id={} pid={:?} dir={} branch={}",
            inst.id,
            inst.pid,
            inst.working_dir.display(),
            branch
        );
    }

    eprintln!("\n=== Codex/OpenCode processes ===");
    let codex = scanner.scan(&["codex"]);
    for p in &codex {
        eprintln!("  PID={} name={} cwd={:?}", p.pid, p.name, p.cwd);
    }
    let opencode = scanner.scan(&["opencode"]);
    for p in &opencode {
        eprintln!("  PID={} name={} cwd={:?}", p.pid, p.name, p.cwd);
    }
}
