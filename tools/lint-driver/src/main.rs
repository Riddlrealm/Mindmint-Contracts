use regex::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

// ─────────────────────────────────────────────────────────────────────────────
// CLI
// ─────────────────────────────────────────────────────────────────────────────

fn usage() {
    eprintln!("Usage: lint-driver [OPTIONS]");
    eprintln!();
    eprintln!("Checks:");
    eprintln!("  --all              Run every check");
    eprintln!("  --event-coverage   pub fns vs event emissions");
    eprintln!("  --missing-auth     pub fns without require_auth");
    eprintln!("  --unsafe-usage     locate unsafe blocks");
    eprintln!("  --feature-flags    surface #[cfg(feature = ...)]");
    eprintln!("  --error-panic-rate #[should_panic] test count per crate");
    eprintln!("  --dead-code        unused fns / types");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --repo-root <DIR>  Repository root (default: cwd)");
    eprintln!("  -h, --help         Show this help");
}

struct Config {
    repo_root: PathBuf,
    checks: Vec<Check>,
}

#[derive(Clone, Debug, PartialEq)]
enum Check {
    EventCoverage,
    MissingAuth,
    UnsafeUsage,
    FeatureFlags,
    ErrorPanicRate,
    DeadCode,
    All,
}

fn parse_args() -> Config {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut checks = Vec::new();
    let mut repo_root = PathBuf::from(".");

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--all" => checks.push(Check::All),
            "--event-coverage" => checks.push(Check::EventCoverage),
            "--missing-auth" => checks.push(Check::MissingAuth),
            "--unsafe-usage" => checks.push(Check::UnsafeUsage),
            "--feature-flags" => checks.push(Check::FeatureFlags),
            "--error-panic-rate" => checks.push(Check::ErrorPanicRate),
            "--dead-code" => checks.push(Check::DeadCode),
            "--repo-root" => {
                i += 1;
                repo_root = PathBuf::from(args.get(i).unwrap_or_else(|| {
                    eprintln!("error: --repo-root requires a path");
                    process::exit(1);
                }));
            }
            "-h" | "--help" => {
                usage();
                process::exit(0);
            }
            other => {
                eprintln!("error: unknown option `{other}`");
                usage();
                process::exit(1);
            }
        }
        i += 1;
    }

    if checks.is_empty() {
        checks.push(Check::All);
    }

    Config { repo_root, checks }
}

// ─────────────────────────────────────────────────────────────────────────────
// Contract discovery
// ─────────────────────────────────────────────────────────────────────────────

struct ContractFile {
    crate_name: String,
    path: PathBuf,
    content: String,
}

fn discover_contracts(repo_root: &Path) -> Vec<ContractFile> {
    let contracts_dir = repo_root.join("contracts");
    let mut result = Vec::new();

    if let Ok(entries) = fs::read_dir(&contracts_dir) {
        for entry in entries.flatten() {
            let lib_rs = entry.path().join("src").join("lib.rs");
            if lib_rs.exists() {
                if let Ok(content) = fs::read_to_string(&lib_rs) {
                    result.push(ContractFile {
                        crate_name: entry.file_name().to_string_lossy().into_owned(),
                        path: lib_rs,
                        content,
                    });
                }
            }
        }
    }

    result.sort_by(|a, b| a.crate_name.cmp(&b.crate_name));
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Check: event-coverage
// ─────────────────────────────────────────────────────────────────────────────

fn check_event_coverage(contracts: &[ContractFile]) -> Vec<String> {
    let pub_fn_re = Regex::new(r"^\s*pub fn ").unwrap();
    let event_re = Regex::new(r"env\.events\(\)\.publish").unwrap();

    let mut findings = Vec::new();
    for c in contracts {
        let pub_count = c.content.lines().filter(|l| pub_fn_re.is_match(l)).count();
        let event_count = c.content.lines().filter(|l| event_re.is_match(l)).count();

        if pub_count > 0 && event_count == 0 {
            findings.push(format!(
                "{}: WARNING – {} public fns but zero events emitted",
                c.crate_name, pub_count
            ));
        } else {
            findings.push(format!(
                "{}: OK – {} public fns, {} event call-sites",
                c.crate_name, pub_count, event_count
            ));
        }
    }
    findings
}

// ─────────────────────────────────────────────────────────────────────────────
// Check: missing-auth
// ─────────────────────────────────────────────────────────────────────────────

fn check_missing_auth(contracts: &[ContractFile]) -> Vec<String> {
    let mut findings = Vec::new();
    for c in contracts {
        if !c.content.contains("require_auth") {
            findings.push(format!("MISSING_AUTH_CHECK: {}", c.crate_name));
        }
    }
    if findings.is_empty() {
        findings.push("All contracts have at least one require_auth call.".into());
    }
    findings
}

// ─────────────────────────────────────────────────────────────────────────────
// Check: unsafe-usage
// ─────────────────────────────────────────────────────────────────────────────

fn check_unsafe_usage(contracts: &[ContractFile]) -> Vec<String> {
    let unsafe_re = Regex::new(r"unsafe\s*\{").unwrap();
    let mut findings = Vec::new();

    for c in contracts {
        for (idx, line) in c.content.lines().enumerate() {
            if unsafe_re.is_match(line) {
                findings.push(format!("{}:{}: unsafe block", c.crate_name, idx + 1));
            }
        }
    }

    if findings.is_empty() {
        findings.push("No unsafe blocks found.".into());
    }
    findings
}

// ─────────────────────────────────────────────────────────────────────────────
// Check: feature-flags
// ─────────────────────────────────────────────────────────────────────────────

fn check_feature_flags(contracts: &[ContractFile]) -> Vec<String> {
    let feature_re = Regex::new(r"cfg\(feature").unwrap();
    let mut findings = Vec::new();

    for c in contracts {
        for (idx, line) in c.content.lines().enumerate() {
            if feature_re.is_match(line) {
                findings.push(format!("{}:{}: {}", c.crate_name, idx + 1, line.trim()));
            }
        }
    }

    if findings.is_empty() {
        findings.push("No feature flags found.".into());
    }
    findings
}

// ─────────────────────────────────────────────────────────────────────────────
// Check: error-panic-rate
// ─────────────────────────────────────────────────────────────────────────────

fn check_error_panic_rate(contracts: &[ContractFile]) -> Vec<String> {
    let mut findings = Vec::new();
    let threshold = 5;

    for c in contracts {
        let count = c.content.matches("should_panic").count();
        let status = if count < threshold {
            "WARN"
        } else {
            "OK"
        };
        findings.push(format!(
            "{}: {} negative tests (threshold: {})",
            c.crate_name, count, threshold
        ));
        if status == "WARN" {
            findings.push(format!(
                "  -> {} has fewer than {} #[should_panic] tests",
                c.crate_name, threshold
            ));
        }
    }
    findings
}

// ─────────────────────────────────────────────────────────────────────────────
// Check: dead-code
// ─────────────────────────────────────────────────────────────────────────────

fn check_dead_code(contracts: &[ContractFile]) -> Vec<String> {
    let fn_re = Regex::new(r"^\s*(?:pub\s+)?fn\s+(\w+)").unwrap();
    let mut findings = Vec::new();

    for c in contracts {
        // Collect all function names
        let fns: Vec<(String, bool)> = c
            .content
            .lines()
            .filter_map(|line| {
                fn_re.captures(line).map(|cap| {
                    let name = cap[1].to_string();
                    let is_pub = line.trim_start().starts_with("pub ");
                    (name, is_pub)
                })
            })
            .collect();

        for (name, is_pub) in &fns {
            // Skip test module functions, main, new
            if name == "new" || name.starts_with("test") || name == "main" {
                continue;
            }

            // Count references to this function name outside its definition
            let def_pattern = format!("fn {name}");
            let call_count = c
                .content
                .lines()
                .filter(|line| !line.trim_start().starts_with("fn ") && line.contains(name))
                .count();

            // A function with zero references (besides its definition) is likely dead
            if call_count == 0 && !is_pub {
                findings.push(format!(
                    "{}: DEAD_CODE – private fn `{}` is never referenced",
                    c.crate_name, name
                ));
            }
        }
    }

    if findings.is_empty() {
        findings.push("No obviously dead private functions found.".into());
    }
    findings
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn run_checks(config: &Config) -> bool {
    let contracts = discover_contracts(&config.repo_root);

    if contracts.is_empty() {
        eprintln!("error: no contracts found under {}", config.repo_root.display());
        return false;
    }

    let run_all = config.checks.contains(&Check::All);
    let mut all_ok = true;

    let run = |check: &Check| run_all || config.checks.contains(check);

    if run(&Check::EventCoverage) {
        println!("═══ Event Coverage ═══");
        let findings = check_event_coverage(&contracts);
        for f in &findings {
            println!("  {f}");
            if f.contains("WARNING") {
                all_ok = false;
            }
        }
        println!();
    }

    if run(&Check::MissingAuth) {
        println!("═══ Missing Auth ═══");
        let findings = check_missing_auth(&contracts);
        for f in &findings {
            println!("  {f}");
            if f.contains("MISSING_AUTH_CHECK") {
                all_ok = false;
            }
        }
        println!();
    }

    if run(&Check::UnsafeUsage) {
        println!("═══ Unsafe Usage ═══");
        let findings = check_unsafe_usage(&contracts);
        for f in &findings {
            println!("  {f}");
        }
        println!();
    }

    if run(&Check::FeatureFlags) {
        println!("═══ Feature Flags ═══");
        let findings = check_feature_flags(&contracts);
        for f in &findings {
            println!("  {f}");
        }
        println!();
    }

    if run(&Check::ErrorPanicRate) {
        println!("═══ Error Panic Rate ═══");
        let findings = check_error_panic_rate(&contracts);
        for f in &findings {
            println!("  {f}");
            if f.contains("WARN") {
                all_ok = false;
            }
        }
        println!();
    }

    if run(&Check::DeadCode) {
        println!("═══ Dead Code ═══");
        let findings = check_dead_code(&contracts);
        for f in &findings {
            println!("  {f}");
            if f.contains("DEAD_CODE") {
                all_ok = false;
            }
        }
        println!();
    }

    all_ok
}

fn main() {
    let config = parse_args();
    let ok = run_checks(&config);

    if !ok {
        eprintln!("lint-driver: some checks reported warnings.");
        process::exit(1);
    }

    println!("lint-driver: all checks passed.");
}
