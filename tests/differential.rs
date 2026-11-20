use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cc() -> String {
    std::env::var("BIFROST_TEST_CC").unwrap_or_else(|_| "clang".to_string())
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bifrost-diff-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bifrost(work: &Path, cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bifrost"))
        .arg(cc())
        .args(args)
        .current_dir(work)
        .env("BIFROST_DIR", cache)
        .env("BIFROST_LOG", "1")
        .output()
        .unwrap()
}

fn plain(work: &Path, args: &[&str]) {
    let st = Command::new(cc())
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
fn depfile_hit_matches_plain_compile() {
    let work = scratch("dep");
    let refdir = scratch("depref");
    let cache = work.join("cache");
    for dir in [&work, &refdir] {
        fs::write(dir.join("a.h"), "#define N 1\n").unwrap();
        fs::write(dir.join("a.c"), "#include \"a.h\"\nint a(void) { return N; }\n").unwrap();
    }
    let flags = ["-c", "a.c", "-MD", "-MF", "a.d", "-o", "a.o"];

    let first = bifrost(&work, &cache, &flags);
    fs::remove_file(work.join("a.o")).unwrap();
    fs::remove_file(work.join("a.d")).unwrap();
    let second = bifrost(&work, &cache, &flags);
    plain(&refdir, &flags);

    assert!(said(&first, "miss"));
    assert!(said(&second, "hit"));
    assert_eq!(fs::read(work.join("a.o")).unwrap(), fs::read(refdir.join("a.o")).unwrap());
    assert_eq!(fs::read(work.join("a.d")).unwrap(), fs::read(refdir.join("a.d")).unwrap());
}

#[test]
fn depfile_target_name_is_part_of_the_key() {
    let work = scratch("deptarget");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { return 1; }\n").unwrap();

    bifrost(&work, &cache, &["-c", "a.c", "-MD", "-MF", "a.d", "-o", "a.o"]);
    let second = bifrost(&work, &cache, &["-c", "a.c", "-MD", "-MF", "b.d", "-o", "b.o"]);

    assert!(said(&second, "miss"));
    let text = fs::read_to_string(work.join("b.d")).unwrap();
    assert!(text.starts_with("b.o:"));
}

#[test]
fn depfile_target_follows_the_object_path() {
    let work = scratch("depobj");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { return 1; }\n").unwrap();

    bifrost(&work, &cache, &["-c", "a.c", "-MD", "-MF", "a.d", "-o", "a.o"]);
    let second = bifrost(&work, &cache, &["-c", "a.c", "-MD", "-MF", "a.d", "-o", "b.o"]);

    assert!(said(&second, "miss"));
    let text = fs::read_to_string(work.join("a.d")).unwrap();
    assert!(text.starts_with("b.o:"));
}

#[test]
fn debug_objects_do_not_leak_the_build_dir() {
    let first = scratch("dbgone");
    let second = scratch("dbgtwo");
    let cache = first.join("cache");
    for dir in [&first, &second] {
        fs::write(dir.join("a.c"), "int a(void) { return 1; }\n").unwrap();
    }
    let flags = ["-g", "-c", "a.c", "-o", "a.o"];

    bifrost(&first, &cache, &flags);
    bifrost(&second, &cache, &flags);
    plain(&second, &["-g", "-c", "a.c", "-o", "real.o"]);

    assert!(same_bytes(&second, "a.o", "real.o"));
}

#[test]
fn debug_build_in_same_dir_still_hits() {
    let work = scratch("dbgsame");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { return 1; }\n").unwrap();

    let one = bifrost(&work, &cache, &["-g", "-c", "a.c", "-o", "one.o"]);
    let two = bifrost(&work, &cache, &["-g", "-c", "a.c", "-o", "two.o"]);

    assert!(said(&one, "miss"));
    assert!(said(&two, "hit"));
}

#[test]
fn stats_counts_hits_and_misses() {
    let work = scratch("stats");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { return 1; }\n").unwrap();

    bifrost(&work, &cache, &["-c", "a.c", "-o", "one.o"]);
    bifrost(&work, &cache, &["-c", "a.c", "-o", "two.o"]);
    bifrost(&work, &cache, &["-c", "a.c", "-o", "three.o"]);

    let out = Command::new(env!("CARGO_BIN_EXE_bifrost"))
        .arg("--stats")
        .env("BIFROST_DIR", &cache)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);

    assert!(text.contains("hits: 2"));
    assert!(text.contains("misses: 1"));
}

#[test]
fn miss_keeps_compiler_warnings() {
    let work = scratch("warn");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { int unused; return 1; }\n").unwrap();
    let flags = ["-Wall", "-c", "a.c", "-o", "a.o"];

    let via_cache = bifrost(&work, &cache, &flags);
    let direct = Command::new(cc()).args(flags).current_dir(&work).output().unwrap();

    let got: Vec<String> = String::from_utf8_lossy(&via_cache.stderr)
        .lines()
        .filter(|l| !l.starts_with("bifrost:"))
        .map(|l| l.to_string())
        .collect();
    let want: Vec<String> = String::from_utf8_lossy(&direct.stderr)
        .lines()
        .map(|l| l.to_string())
        .collect();

    assert!(!want.is_empty());
    assert_eq!(got, want);
}

#[test]
fn file_macro_matches_plain_compile() {
    let work = scratch("filemacro");
    let refdir = scratch("filemacroref");
    let cache = work.join("cache");
    for dir in [&work, &refdir] {
        fs::write(dir.join("a.c"), "const char *f(void) { return __FILE__; }\n").unwrap();
    }

    bifrost(&work, &cache, &["-c", "a.c", "-o", "a.o"]);
    plain(&refdir, &["-c", "a.c", "-o", "a.o"]);

    assert_eq!(fs::read(work.join("a.o")).unwrap(), fs::read(refdir.join("a.o")).unwrap());
}

#[test]
fn response_file_contents_are_part_of_the_key() {
    let work = scratch("rsp");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(int x) { return x * 2 + x * 2; }\n").unwrap();

    fs::write(work.join("flags.rsp"), "-O0\n").unwrap();
    bifrost(&work, &cache, &["-c", "a.c", "-o", "slow.o", "@flags.rsp"]);

    fs::write(work.join("flags.rsp"), "-O2\n").unwrap();
    let fast = bifrost(&work, &cache, &["-c", "a.c", "-o", "fast.o", "@flags.rsp"]);
    plain(&work, &["-c", "a.c", "-o", "real_fast.o", "@flags.rsp"]);

    assert!(said(&fast, "miss"));
    assert!(same_bytes(&work, "fast.o", "real_fast.o"));
}

#[test]
fn mmd_without_mf_hits_and_matches_plain() {
    let work = scratch("mmd");
    let refdir = scratch("mmdref");
    let cache = work.join("cache");
    for dir in [&work, &refdir] {
        fs::write(dir.join("a.h"), "#define N 1\n").unwrap();
        fs::write(dir.join("a.c"), "#include \"a.h\"\nint a(void) { return N; }\n").unwrap();
    }
    let flags = ["-c", "a.c", "-MMD", "-o", "a.o"];

    let first = bifrost(&work, &cache, &flags);
    fs::remove_file(work.join("a.o")).unwrap();
    fs::remove_file(work.join("a.d")).unwrap();
    let second = bifrost(&work, &cache, &flags);
    plain(&refdir, &flags);

    assert!(said(&first, "miss"));
    assert!(said(&second, "hit"));
    assert_eq!(fs::read(work.join("a.o")).unwrap(), fs::read(refdir.join("a.o")).unwrap());
    assert_eq!(fs::read(work.join("a.d")).unwrap(), fs::read(refdir.join("a.d")).unwrap());
}
