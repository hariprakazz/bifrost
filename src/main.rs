mod args;
mod key;
mod store;

use std::env;
use std::process::{exit, Command};

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

fn wants_depfile(rest: &[String]) -> bool {
    rest.iter()
        .any(|a| matches!(a.as_str(), "-MD" | "-MMD" | "-MF" | "-M" | "-MM"))
}

fn note(msg: &str) {
    if env::var_os("BIFROST_LOG").is_some() {
        eprintln!("bifrost: {}", msg);
    }
}

fn try_cached(compiler: &str, rest: &[String]) -> Option<i32> {
    if wants_depfile(rest) {
        return None;
    }
    let job = args::parse(rest)?;
    let pre = key::preprocessed(compiler, rest)?;
    let id = key::cache_key(compiler, rest, &pre)?;
    let store = Store::new(Store::default_root()?);

    if let Ok(true) = store.fetch(&id, &job.obj) {
        note("hit");
        return Some(0);
    }

    note("miss");
    let code = run_real(compiler, rest);
    if code == 0 {
        if let Err(err) = store.put(&id, &job.obj) {
            eprintln!("bifrost: cache write failed: {}", err);
        }
    }
    Some(code)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some((compiler, rest)) = args.split_first() else {
        eprintln!("usage: bifrost <compiler> [args...]");
        exit(2);
    };

    let code = try_cached(compiler, rest).unwrap_or_else(|| run_real(compiler, rest));
    exit(code);
}
