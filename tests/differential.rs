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
fn md_without_mf_skips_the_cache() {
    let work = scratch("depskip");
    let cache = work.join("cache");
    fs::write(work.join("a.c"), "int a(void) { return 1; }\n").unwrap();

    let first = bifrost(&work, &cache, &["-c", "a.c", "-MD", "-o", "a.o"]);

    assert!(first.status.success());
    assert!(!said(&first, "hit") && !said(&first, "miss"));
    assert!(work.join("a.d").is_file());
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
