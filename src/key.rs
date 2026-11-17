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

pub fn cache_key(compiler: &str, rest: &[String], pre: &[u8]) -> Option<String> {
    let ver = compiler_version(compiler)?;
    let mut hasher = blake3::Hasher::new();
    let mut feed = |part: &[u8]| {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    };

    feed(&ver);
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        if arg == "-o" {
            iter.next();
            continue;
        }
        feed(arg.as_bytes());
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
}
