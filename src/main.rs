mod args;
mod key;
mod stats;
mod store;

use std::env;
use std::io::Write;
use std::process::{exit, Command, Stdio};

use store::Store;

fn run_real(compiler: &str, rest: &[String]) -> i32 {
    match Command::new(compiler).args(rest).status() {
        Ok(st) => st.code().unwrap_or(1),
        Err(err) => {
            eprintln!("bifrost: could not run {}: {}", compiler, err);
            127
        }
    }
}

fn run_capture(compiler: &str, rest: &[String]) -> (i32, Vec<u8>) {
    let out = Command::new(compiler)
        .args(rest)
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .output();
    match out {
        Ok(out) => (out.status.code().unwrap_or(1), out.stderr),
        Err(err) => {
            eprintln!("bifrost: could not run {}: {}", compiler, err);
            (127, Vec::new())
        }
    }
}

fn note(msg: &str) {
    if env::var_os("BIFROST_LOG").is_some() {
        eprintln!("bifrost: {}", msg);
    }
}

fn try_cached(compiler: &str, rest: &[String]) -> Option<i32> {
    let flat = args::expand(rest)?;
    let job = args::parse(&flat)?;
    let pre = key::preprocessed(compiler, &flat)?;
    let id = key::cache_key(compiler, &flat, &pre)?;
    let root = Store::default_root()?;
    let store = Store::new(root.clone());
    let dep_id = format!("{}.dep", id);
    let err_id = format!("{}.err", id);
    let saved_err = store.read_bytes(&err_id);

    if let Ok(true) = store.fetch(&id, &job.obj) {
        let dep_ok = match &job.dep {
            Some(dep) => matches!(store.fetch(&dep_id, dep), Ok(true)),
            None => true,
        };
        if let (true, Some(text)) = (dep_ok, &saved_err) {
            let _ = std::io::stderr().write_all(text);
            note("hit");
            stats::record(&root, true);
            return Some(0);
        }
    }

    note("miss");
    stats::record(&root, false);
    let (code, text) = run_capture(compiler, rest);
    let _ = std::io::stderr().write_all(&text);
    if code == 0 {
        let dep_saved = match &job.dep {
            Some(dep) => store.put(&dep_id, dep),
            None => Ok(()),
        };
        let err_saved = store.write_bytes(&err_id, &text);
        let saved = err_saved.and_then(|_| dep_saved).and_then(|_| store.put(&id, &job.obj));
        if let Err(err) = saved {
            eprintln!("bifrost: cache write failed: {}", err);
        }
        store.trim(Store::limit_bytes());
    }
    Some(code)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--stats") {
        let Some(root) = Store::default_root() else {
            eprintln!("bifrost: no cache directory");
            exit(1);
        };
        let (hits, misses) = stats::totals(&root);
        println!("hits: {}\nmisses: {}", hits, misses);
        exit(0);
    }

    let Some((compiler, rest)) = args.split_first() else {
        eprintln!("usage: bifrost <compiler> [args...]");
        exit(2);
    };

    let code = try_cached(compiler, rest).unwrap_or_else(|| run_real(compiler, rest));
    exit(code);
}
