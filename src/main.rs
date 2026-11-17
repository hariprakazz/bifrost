mod args;
mod key;
mod store;

use std::env;
use std::process::{exit, Command};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some((compiler, rest)) = args.split_first() else {
        eprintln!("usage: bifrost <compiler> [args...]");
        exit(2);
    };

    let st = match Command::new(compiler).args(rest).status() {
        Ok(st) => st,
        Err(err) => {
            eprintln!("bifrost: could not run {}: {}", compiler, err);
            exit(127);
        }
    };

    exit(st.code().unwrap_or(1));
}
