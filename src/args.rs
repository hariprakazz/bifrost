use std::path::PathBuf;

pub struct Job {
    pub src: PathBuf,
    pub obj: PathBuf,
}

const SRC_EXT: [&str; 4] = ["c", "cc", "cpp", "cxx"];

pub fn parse(rest: &[String]) -> Option<Job> {
    let mut src: Option<PathBuf> = None;
    let mut obj: Option<PathBuf> = None;
    let mut compile_only = false;
    let mut iter = rest.iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-c" => compile_only = true,
            "-o" => obj = Some(PathBuf::from(iter.next()?)),
            _ if arg.starts_with('-') => {}
            _ => {
                let path = PathBuf::from(arg);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if SRC_EXT.contains(&ext) {
                    if src.is_some() {
                        return None;
                    }
                    src = Some(path);
                }
            }
        }
    }

    if !compile_only {
        return None;
    }
    Some(Job { src: src?, obj: obj? })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn plain_compile() {
        let job = parse(&argv(&["-c", "foo.c", "-o", "foo.o"])).unwrap();
        assert_eq!(job.src, PathBuf::from("foo.c"));
        assert_eq!(job.obj, PathBuf::from("foo.o"));
    }

    #[test]
    fn flags_in_between() {
        let job = parse(&argv(&["-O2", "-Iinc", "-c", "a.cpp", "-DX=1", "-o", "out/a.o"])).unwrap();
        assert_eq!(job.src, PathBuf::from("a.cpp"));
        assert_eq!(job.obj, PathBuf::from("out/a.o"));
    }

    #[test]
    fn link_step_is_skipped() {
        assert!(parse(&argv(&["foo.o", "-o", "app"])).is_none());
    }

    #[test]
    fn two_sources_are_skipped() {
        assert!(parse(&argv(&["-c", "a.c", "b.c", "-o", "x.o"])).is_none());
    }

    #[test]
    fn missing_output_is_skipped() {
        assert!(parse(&argv(&["-c", "foo.c"])).is_none());
    }
}
