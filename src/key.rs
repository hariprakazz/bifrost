use std::process::Command;

pub fn preprocessed(compiler: &str, rest: &[String]) -> Option<Vec<u8>> {
    let mut pp: Vec<&str> = Vec::new();
    let mut iter = rest.iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-c" | "-MD" | "-MMD" | "-MP" => {}
            "-o" | "-MF" | "-MT" | "-MQ" => {
                iter.next();
            }
            other => pp.push(other),
        }
    }

    let out = Command::new(compiler).args(&pp).arg("-E").output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(out.stdout)
}

#[cfg(test)]
pub fn digest(pre: &[u8]) -> String {
    blake3::hash(pre).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bifrost-{}-{}", name, std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn argv(src: &PathBuf) -> Vec<String> {
        vec![
            "-c".to_string(),
            src.display().to_string(),
            "-o".to_string(),
            "x.o".to_string(),
        ]
    }

    #[test]
    fn same_source_same_digest() {
        let dir = scratch("same");
        let src = dir.join("a.c");
        fs::write(&src, "int a(void) { return 1; }\n").unwrap();
        let one = preprocessed("clang", &argv(&src)).unwrap();
        let two = preprocessed("clang", &argv(&src)).unwrap();
        assert_eq!(digest(&one), digest(&two));
    }

    #[test]
    fn header_change_changes_digest() {
        let dir = scratch("hdr");
        let src = dir.join("a.c");
        let hdr = dir.join("a.h");
        fs::write(&src, "#include \"a.h\"\nint a(void) { return N; }\n").unwrap();
        fs::write(&hdr, "#define N 1\n").unwrap();
        let before = digest(&preprocessed("clang", &argv(&src)).unwrap());
        fs::write(&hdr, "#define N 2\n").unwrap();
        let after = digest(&preprocessed("clang", &argv(&src)).unwrap());
        assert_ne!(before, after);
    }

    #[test]
    fn broken_source_gives_none() {
        let dir = scratch("bad");
        let src = dir.join("a.c");
        fs::write(&src, "#include \"missing.h\"\n").unwrap();
        assert!(preprocessed("clang", &argv(&src)).is_none());
    }
}

fn compiler_version(compiler: &str) -> Option<Vec<u8>> {
    let out = Command::new(compiler).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(out.stdout)
}

fn compiler_identity(compiler: &str, ver: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(ver);
    let dir = text.lines().find_map(|l| l.strip_prefix("InstalledDir: "));
    let name = std::path::Path::new(compiler).file_name();
    let (Some(dir), Some(name)) = (dir, name) else {
        return Vec::new();
    };
    let bin = std::path::Path::new(dir.trim()).join(name);
    let Ok(meta) = std::fs::metadata(&bin) else {
        return Vec::new();
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}:{}:{}", bin.display(), meta.len(), mtime).into_bytes()
}

const ENV_KEYS: [&str; 3] = [
    "MACOSX_DEPLOYMENT_TARGET",
    "IPHONEOS_DEPLOYMENT_TARGET",
    "SDKROOT",
];

pub fn cache_key(compiler: &str, rest: &[String], pre: &[u8]) -> Option<String> {
    let ver = compiler_version(compiler)?;
    let mut hasher = blake3::Hasher::new();
    let mut feed = |part: &[u8]| {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    };

    feed(&ver);
    feed(&compiler_identity(compiler, &ver));
    let with_dep = rest.iter().any(|a| a == "-MD" || a == "-MMD");
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        if arg == "-o" && !with_dep {
            iter.next();
            continue;
        }
        feed(arg.as_bytes());
    }
    if rest.iter().any(|a| a.starts_with("-g")) {
        let cwd = std::env::current_dir().ok()?;
        feed(cwd.to_string_lossy().as_bytes());
    }
    for name in ENV_KEYS {
        let val = std::env::var(name).unwrap_or_default();
        feed(format!("{}={}", name, val).as_bytes());
    }
    feed(pre);

    Some(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod flag_tests {
    use super::*;
    use std::fs;

    fn argv(src: &str, opt: &str) -> Vec<String> {
        vec![
            opt.to_string(),
            "-c".to_string(),
            src.to_string(),
            "-o".to_string(),
            "x.o".to_string(),
        ]
    }

    #[test]
    fn opt_level_changes_key() {
        let dir = std::env::temp_dir().join(format!("bifrost-opt-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.c");
        fs::write(&src, "int a(void) { return 1; }\n").unwrap();
        let path = src.display().to_string();
        let slow = argv(&path, "-O0");
        let fast = argv(&path, "-O2");
        let pre_slow = preprocessed("clang", &slow).unwrap();
        let pre_fast = preprocessed("clang", &fast).unwrap();
        assert_eq!(pre_slow, pre_fast);
        assert_ne!(
            cache_key("clang", &slow, &pre_slow),
            cache_key("clang", &fast, &pre_fast)
        );
    }

    #[test]
    fn compiler_binary_change_changes_key() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("bifrost-id-{}", std::process::id()));
        let bin_dir = dir.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let real = bin_dir.join("clang");
        let shim_dir = dir.join("shim");
        fs::create_dir_all(&shim_dir).unwrap();
        let shim = shim_dir.join("clang");
        let script = format!(
            "#!/bin/sh\necho 'fake clang version 1'\necho 'InstalledDir: {}'\n",
            bin_dir.display()
        );
        fs::write(&shim, script).unwrap();
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        let compiler = shim.display().to_string();
        let rest = argv("a.c", "-O0");
        fs::write(&real, "one").unwrap();
        let before = cache_key(&compiler, &rest, b"pre");
        fs::write(&real, "two plus").unwrap();
        let after = cache_key(&compiler, &rest, b"pre");
        assert!(before.is_some());
        assert_ne!(before, after);
    }

    #[test]
    fn compiler_mtime_change_changes_key() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{Duration, SystemTime};
        let dir = std::env::temp_dir().join(format!("bifrost-mt-{}", std::process::id()));
        let bin_dir = dir.join("bin");
        let shim_dir = dir.join("shim");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&shim_dir).unwrap();
        let real = bin_dir.join("clang");
        let shim = shim_dir.join("clang");
        let script = format!(
            "#!/bin/sh\necho 'fake clang version 1'\necho 'InstalledDir: {}'\n",
            bin_dir.display()
        );
        fs::write(&shim, script).unwrap();
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        let compiler = shim.display().to_string();
        let rest = argv("a.c", "-O0");
        fs::write(&real, "same").unwrap();
        let file = fs::OpenOptions::new().write(true).open(&real).unwrap();
        let stamp = SystemTime::now() - Duration::from_secs(3600);
        file.set_modified(stamp).unwrap();
        let before = cache_key(&compiler, &rest, b"pre");
        file.set_modified(stamp + Duration::from_secs(60)).unwrap();
        let after = cache_key(&compiler, &rest, b"pre");
        assert!(before.is_some());
        assert_ne!(before, after);
    }

    #[test]
    fn compiler_size_change_changes_key() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{Duration, SystemTime};
        let dir = std::env::temp_dir().join(format!("bifrost-sz-{}", std::process::id()));
        let bin_dir = dir.join("bin");
        let shim_dir = dir.join("shim");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&shim_dir).unwrap();
        let real = bin_dir.join("clang");
        let shim = shim_dir.join("clang");
        let script = format!(
            "#!/bin/sh\necho 'fake clang version 1'\necho 'InstalledDir: {}'\n",
            bin_dir.display()
        );
        fs::write(&shim, script).unwrap();
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        let compiler = shim.display().to_string();
        let rest = argv("a.c", "-O0");
        let stamp = SystemTime::now() - Duration::from_secs(3600);
        fs::write(&real, "same").unwrap();
        let file = fs::OpenOptions::new().write(true).open(&real).unwrap();
        file.set_modified(stamp).unwrap();
        let before = cache_key(&compiler, &rest, b"pre");
        fs::write(&real, "same but longer").unwrap();
        let file = fs::OpenOptions::new().write(true).open(&real).unwrap();
        file.set_modified(stamp).unwrap();
        let after = cache_key(&compiler, &rest, b"pre");
        assert!(before.is_some());
        assert_ne!(before, after);
    }
}
