use std::path::PathBuf;

pub struct Job {
    pub src: PathBuf,
    pub obj: PathBuf,
    pub dep: Option<PathBuf>,
}

const SRC_EXT: [&str; 4] = ["c", "cc", "cpp", "cxx"];

const BYPASS: [&str; 13] = [
    "-ftime-trace",
    "--serialize-diagnostics",
    "-MJ",
    "-save-temps",
    "-fsave-optimization-record",
    "-fprofile-use",
    "-fprofile-sample-use",
    "-fsanitize-ignorelist",
    "-fsanitize-blacklist",
    "--coverage",
    "-fprofile-arcs",
    "-ftest-coverage",
    "-fprofile-generate",
];

pub fn parse(rest: &[String]) -> Option<Job> {
    let mut src: Option<PathBuf> = None;
    let mut obj: Option<PathBuf> = None;
    let mut dep_out: Option<PathBuf> = None;
    let mut compile_only = false;
    let mut want_dep = false;
    let mut iter = rest.iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-c" => compile_only = true,
            "-o" => obj = Some(PathBuf::from(iter.next()?)),
            "-MD" | "-MMD" => want_dep = true,
            "-MF" => dep_out = Some(PathBuf::from(iter.next()?)),
            "-M" | "-MM" => return None,
            _ if BYPASS.iter().any(|p| arg.starts_with(p)) => return None,
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
    if want_dep != dep_out.is_some() {
        return None;
    }
    Some(Job { src: src?, obj: obj?, dep: dep_out })
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
        assert!(job.dep.is_none());
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

    #[test]
    fn depfile_with_explicit_path() {
        let job = parse(&argv(&["-c", "a.c", "-MD", "-MF", "a.d", "-o", "a.o"])).unwrap();
        assert_eq!(job.dep, Some(PathBuf::from("a.d")));
    }

    #[test]
    fn md_without_mf_is_skipped() {
        assert!(parse(&argv(&["-c", "a.c", "-MD", "-o", "a.o"])).is_none());
    }

    #[test]
    fn mf_without_md_is_skipped() {
        assert!(parse(&argv(&["-c", "a.c", "-MF", "a.d", "-o", "a.o"])).is_none());
    }

    #[test]
    fn extra_output_flags_are_skipped() {
        for flag in BYPASS {
            assert!(parse(&argv(&["-c", "a.c", flag, "-o", "a.o"])).is_none());
        }
    }

    #[test]
    fn coverage_flags_are_skipped() {
        for flag in ["--coverage", "-fprofile-arcs", "-ftest-coverage", "-fprofile-generate"] {
            assert!(parse(&argv(&["-c", "a.c", flag, "-o", "a.o"])).is_none());
        }
    }

    #[test]
    fn preprocess_only_dep_modes_are_skipped() {
        assert!(parse(&argv(&["-c", "a.c", "-M", "-o", "a.o"])).is_none());
        assert!(parse(&argv(&["-c", "a.c", "-MM", "-o", "a.o"])).is_none());
    }
}
