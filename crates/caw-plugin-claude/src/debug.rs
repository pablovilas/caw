use crate::process;
use caw_core::ProcessScanner;
use std::time::Instant;

pub fn debug_processes() {
    let mut scanner = ProcessScanner::new();

    let t0 = Instant::now();
    let procs = scanner.scan(&["claude"]);
    let d1 = t0.elapsed();

    let t1 = Instant::now();
    let _ = scanner.scan(&["codex"]);
    let d2 = t1.elapsed();

    let t2 = Instant::now();
    let _ = scanner.scan(&["opencode"]);
    let d3 = t2.elapsed();

    eprintln!("=== Scan times ===");
    eprintln!("  1st (claude):   {:?}", d1);
    eprintln!("  2nd (codex):    {:?}", d2);
    eprintln!("  3rd (opencode): {:?}", d3);

    eprintln!("\n=== Running Claude processes ===");
    for p in &procs {
        eprintln!("  PID={} cwd={:?}", p.pid, p.cwd);
    }

    eprintln!("\n=== Discovered Claude instances ===");
    let instances = process::discover_claude_instances(procs);
    for inst in &instances {
        let branch = inst.extra.get("git_branch").and_then(|v| v.as_str()).unwrap_or("-");
        let app = inst.extra.get("app_name").and_then(|v| v.as_str()).unwrap_or("-");
        eprintln!("  pid={:?} dir={} branch={} app={}", inst.pid, inst.working_dir.display(), branch, app);
    }
}
