use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn touch(path: &Path) {
    if let Ok(file) = fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

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
        let dir = dest.parent().unwrap_or(Path::new(""));
        let tmp = dir.join(format!(".bifrost-{}.tmp", std::process::id()));
        let copied = fs::copy(&slot, &tmp).map(|_| ()).and_then(|_| {
            let file = fs::OpenOptions::new().write(true).open(&tmp)?;
            file.set_modified(SystemTime::now())
        });
        if let Err(err) = copied {
            let _ = fs::remove_file(&tmp);
            return Err(err);
        }
        fs::rename(&tmp, dest)?;
        touch(&slot);
        Ok(true)
    }

    pub fn limit_bytes() -> u64 {
        let mb = env::var("BIFROST_MAX_MB")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5120);
        mb * 1024 * 1024
    }

    pub fn trim(&self, limit: u64) {
        let Ok(shards) = fs::read_dir(&self.root) else {
            return;
        };
        let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        let mut total: u64 = 0;
        for shard in shards.flatten() {
            if !shard.path().is_dir() {
                continue;
            }
            let Ok(entries) = fs::read_dir(shard.path()) else {
                continue;
            };
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let Ok(meta) = entry.metadata() else { continue };
                let Ok(stamp) = meta.modified() else { continue };
                total += meta.len();
                files.push((stamp, meta.len(), entry.path()));
            }
        }
        if total <= limit {
            return;
        }
        files.sort();
        let target = limit / 10 * 9;
        for (_, size, path) in files {
            if total <= target {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total -= size;
            }
        }
    }

    pub fn read_bytes(&self, key: &str) -> Option<Vec<u8>> {
        let slot = self.slot(key);
        let data = fs::read(&slot).ok()?;
        touch(&slot);
        Some(data)
    }

    pub fn write_bytes(&self, key: &str, data: &[u8]) -> io::Result<()> {
        let slot = self.slot(key);
        let parent = slot.parent().unwrap();
        fs::create_dir_all(parent)?;
        let tmp = parent.join(format!(".{}.{}", key, std::process::id()));
        fs::write(&tmp, data)?;
        fs::rename(&tmp, &slot)
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
    fn fetch_leaves_no_temp_files() {
        let dir = scratch("fetchtmp");
        let store = Store::new(dir.join("cache"));
        let obj = dir.join("in.o");
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(&obj, b"data").unwrap();
        store.put(KEY, &obj).unwrap();
        assert!(store.fetch(KEY, &out.join("a.o")).unwrap());
        let names: Vec<String> = fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["a.o".to_string()]);
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

#[cfg(test)]
mod race_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    const KEY: &str = "cd34c0ffee00000000000000000000000000000000000000000000000000beef";

    #[test]
    fn reader_never_sees_a_partial_object() {
        let dir = env::temp_dir().join(format!("bifrost-race-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let obj = dir.join("big.o");
        let size = 4 * 1024 * 1024;
        fs::write(&obj, vec![7u8; size]).unwrap();

        let store = Arc::new(Store::new(dir.join("cache")));
        store.put(KEY, &obj).unwrap();
        let done = Arc::new(AtomicBool::new(false));

        let writer = {
            let store = Arc::clone(&store);
            let done = Arc::clone(&done);
            let obj = obj.clone();
            thread::spawn(move || {
                for _ in 0..40 {
                    store.put(KEY, &obj).unwrap();
                }
                done.store(true, Ordering::SeqCst);
            })
        };

        let mut seen = 0;
        while !done.load(Ordering::SeqCst) {
            let dest = dir.join("got.o");
            if store.fetch(KEY, &dest).unwrap() {
                assert_eq!(fs::metadata(&dest).unwrap().len() as usize, size);
                seen += 1;
            }
        }
        writer.join().unwrap();
        assert!(seen > 0);
    }
}

#[cfg(test)]
mod trim_tests {
    use super::*;
    use std::time::Duration;

    fn scratch(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("bifrost-trim-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn age(path: &Path, secs: u64) {
        let file = fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(secs)).unwrap();
    }

    fn seed(store: &Store, dir: &Path, key: &str, size: usize) {
        let obj = dir.join("seed.o");
        fs::write(&obj, vec![1u8; size]).unwrap();
        store.put(key, &obj).unwrap();
    }

    #[test]
    fn oldest_goes_first() {
        let dir = scratch("order");
        let store = Store::new(dir.join("cache"));
        seed(&store, &dir, "aa-old", 1000);
        seed(&store, &dir, "bb-mid", 1000);
        seed(&store, &dir, "cc-new", 1000);
        age(&store.slot("aa-old"), 300);
        age(&store.slot("bb-mid"), 200);
        age(&store.slot("cc-new"), 100);
        store.trim(2500);
        assert!(!store.slot("aa-old").exists());
        assert!(store.slot("bb-mid").exists());
        assert!(store.slot("cc-new").exists());
    }

    #[test]
    fn under_the_limit_nothing_goes() {
        let dir = scratch("under");
        let store = Store::new(dir.join("cache"));
        seed(&store, &dir, "aa-old", 1000);
        seed(&store, &dir, "bb-mid", 1000);
        store.trim(5000);
        assert!(store.slot("aa-old").exists());
        assert!(store.slot("bb-mid").exists());
    }

    #[test]
    fn temp_files_are_left_alone() {
        let dir = scratch("stray");
        let store = Store::new(dir.join("cache"));
        seed(&store, &dir, "aa-old", 1000);
        let stray = store.slot("aa-old").parent().unwrap().join(".aa-old.999");
        fs::write(&stray, vec![0u8; 5000]).unwrap();
        store.trim(1500);
        assert!(store.slot("aa-old").exists());
        assert!(stray.exists());
    }

    #[test]
    fn fetch_refreshes_the_age() {
        let dir = scratch("touch");
        let store = Store::new(dir.join("cache"));
        seed(&store, &dir, "aa-old", 1000);
        age(&store.slot("aa-old"), 3600);
        let before = fs::metadata(store.slot("aa-old")).unwrap().modified().unwrap();
        store.fetch("aa-old", &dir.join("out.o")).unwrap();
        let after = fs::metadata(store.slot("aa-old")).unwrap().modified().unwrap();
        assert!(after > before + Duration::from_secs(1800));
    }
}
