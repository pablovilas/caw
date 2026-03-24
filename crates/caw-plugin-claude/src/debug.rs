use crate::process;

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
}
