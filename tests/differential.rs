use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bifrost-diff-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bifrost(work: &Path, cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bifrost"))
        .arg("clang")
        .args(args)
        .current_dir(work)
        .env("BIFROST_DIR", cache)
        .env("BIFROST_LOG", "1")
        .output()
        .unwrap()
}

fn plain(work: &Path, args: &[&str]) {
    let st = Command::new("clang")
        .args(args)
        .current_dir(work)
        .status()
        .unwrap();
    assert!(st.success());
}

fn said(out: &Output, word: &str) -> bool {
    String::from_utf8_lossy(&out.stderr).contains(&format!("bifrost: {}", word))
}

fn same_bytes(work: &Path, a: &str, b: &str) -> bool {
    fs::read(work.join(a)).unwrap() == fs::read(work.join(b)).unwrap()
}

#[test]
fn hit_matches_plain_compile() {
    let work = scratch("hit");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { return 1; }\n").unwrap();

    let first = bifrost(&work, &cache, &["-c", "a.c", "-o", "one.o"]);
    let second = bifrost(&work, &cache, &["-c", "a.c", "-o", "two.o"]);
    plain(&work, &["-c", "a.c", "-o", "real.o"]);

    assert!(said(&first, "miss"));
    assert!(said(&second, "hit"));
    assert!(same_bytes(&work, "one.o", "real.o"));
    assert!(same_bytes(&work, "two.o", "real.o"));
}

#[test]
fn header_change_is_a_miss() {
    let work = scratch("header");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "#include \"a.h\"\nint a(void) { return N; }\n").unwrap();
    fs::write(work.join("a.h"), "#define N 1\n").unwrap();

    let first = bifrost(&work, &cache, &["-c", "a.c", "-o", "one.o"]);
    let second = bifrost(&work, &cache, &["-c", "a.c", "-o", "two.o"]);
    fs::write(work.join("a.h"), "#define N 2\n").unwrap();
    let third = bifrost(&work, &cache, &["-c", "a.c", "-o", "three.o"]);
    plain(&work, &["-c", "a.c", "-o", "real.o"]);

    assert!(said(&first, "miss"));
    assert!(said(&second, "hit"));
    assert!(said(&third, "miss"));
    assert!(same_bytes(&work, "three.o", "real.o"));
    assert!(!same_bytes(&work, "one.o", "three.o"));
}

#[test]
fn opt_levels_do_not_share_entries() {
    let work = scratch("opt");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(int x) { return x * 2 + x * 2; }\n").unwrap();

    let slow = bifrost(&work, &cache, &["-O0", "-c", "a.c", "-o", "slow.o"]);
    let fast = bifrost(&work, &cache, &["-O2", "-c", "a.c", "-o", "fast.o"]);
    plain(&work, &["-O0", "-c", "a.c", "-o", "real_slow.o"]);
    plain(&work, &["-O2", "-c", "a.c", "-o", "real_fast.o"]);

    assert!(said(&slow, "miss"));
    assert!(said(&fast, "miss"));
    assert!(same_bytes(&work, "slow.o", "real_slow.o"));
    assert!(same_bytes(&work, "fast.o", "real_fast.o"));
}

#[test]
fn failed_compile_is_not_cached() {
    let work = scratch("fail");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { return 1 }\n").unwrap();

    let first = bifrost(&work, &cache, &["-c", "a.c", "-o", "a.o"]);
    let second = bifrost(&work, &cache, &["-c", "a.c", "-o", "a.o"]);

    assert!(!first.status.success());
    assert!(!second.status.success());
    assert!(said(&first, "miss"));
    assert!(said(&second, "miss"));
}

#[test]
fn depfile_runs_skip_the_cache() {
    let work = scratch("dep");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { return 1; }\n").unwrap();

    let first = bifrost(&work, &cache, &["-c", "a.c", "-MD", "-MF", "a.d", "-o", "a.o"]);
    let second = bifrost(&work, &cache, &["-c", "a.c", "-MD", "-MF", "a.d", "-o", "a.o"]);

    assert!(first.status.success());
    assert!(second.status.success());
    assert!(!said(&first, "hit") && !said(&first, "miss"));
    assert!(!said(&second, "hit") && !said(&second, "miss"));
    assert!(work.join("a.d").is_file());
}
