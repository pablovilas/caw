use crate::process;

pub fn debug_processes() {
    let procs = process::get_claude_processes();
    eprintln!("=== Running Claude processes ===");
    for p in &procs {
        eprintln!("  PID={} cwd={:?}", p.pid, p.cwd);
    }

    eprintln!("\n=== Discovered instances ===");
    let instances = process::discover_claude_instances();
    for inst in &instances {
        eprintln!(
            "  id={} pid={:?} working_dir={} project_name={}",
            inst.id,
            inst.pid,
            inst.working_dir.display(),
            inst.working_dir.file_name().unwrap_or_default().to_string_lossy()
        );
    }
}
