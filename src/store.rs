use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Store {
        Store { root }
    }

    pub fn default_root() -> Option<PathBuf> {
        if let Some(dir) = env::var_os("BIFROST_DIR") {
            return Some(PathBuf::from(dir));
        }
        let home = env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".cache").join("bifrost"))
    }

    fn slot(&self, key: &str) -> PathBuf {
        self.root.join(&key[..2]).join(key)
    }

    pub fn fetch(&self, key: &str, dest: &Path) -> io::Result<bool> {
        let slot = self.slot(key);
        if !slot.is_file() {
            return Ok(false);
        }
        fs::copy(&slot, dest)?;
        Ok(true)
    }

    pub fn put(&self, key: &str, obj: &Path) -> io::Result<()> {
        let slot = self.slot(key);
        let parent = slot.parent().unwrap();
        fs::create_dir_all(parent)?;
        let tmp = parent.join(format!(".{}.{}", key, std::process::id()));
        fs::copy(obj, &tmp)?;
        fs::rename(&tmp, &slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "ab34c0ffee00000000000000000000000000000000000000000000000000beef";

    fn scratch(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("bifrost-store-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn miss_on_empty_store() {
        let dir = scratch("miss");
        let store = Store::new(dir.join("cache"));
        let dest = dir.join("out.o");
        assert!(!store.fetch(KEY, &dest).unwrap());
        assert!(!dest.exists());
    }

    #[test]
    fn put_then_fetch_gives_same_bytes() {
        let dir = scratch("roundtrip");
        let store = Store::new(dir.join("cache"));
        let obj = dir.join("in.o");
        let dest = dir.join("out.o");
        fs::write(&obj, b"\x7fELFfake object bytes").unwrap();
        store.put(KEY, &obj).unwrap();
        assert!(store.fetch(KEY, &dest).unwrap());
        assert_eq!(fs::read(&obj).unwrap(), fs::read(&dest).unwrap());
    }

    #[test]
    fn put_leaves_no_temp_files() {
        let dir = scratch("tmp");
        let store = Store::new(dir.join("cache"));
        let obj = dir.join("in.o");
        fs::write(&obj, b"data").unwrap();
        store.put(KEY, &obj).unwrap();
        let names: Vec<String> = fs::read_dir(dir.join("cache").join("ab"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec![KEY.to_string()]);
    }
}
