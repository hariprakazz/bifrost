use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

const LOG: &str = "events.log";

pub fn record(root: &Path, hit: bool) {
    let _ = fs::create_dir_all(root);
    let line: &[u8] = if hit { b"h\n" } else { b"m\n" };
    if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(root.join(LOG)) {
        let _ = log.write_all(line);
    }
}

pub fn totals(root: &Path) -> (usize, usize) {
    let text = fs::read_to_string(root.join(LOG)).unwrap_or_default();
    let hits = text.lines().filter(|l| *l == "h").count();
    let misses = text.lines().filter(|l| *l == "m").count();
    (hits, misses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_event() {
        let dir = std::env::temp_dir().join(format!("bifrost-stats-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(totals(&dir), (0, 0));
        record(&dir, false);
        record(&dir, true);
        record(&dir, true);
        assert_eq!(totals(&dir), (2, 1));
    }
}
