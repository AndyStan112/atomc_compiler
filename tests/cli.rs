use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "atomc-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn source(&self, text: &str) -> PathBuf {
        let path = self.0.join("program.atomc");
        fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_AtomC"))
}
#[test]
fn interactive_example_flushes_prompt_before_reading_input() {
    let mut child = cli()
        .arg("examples/sum.atomc")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    out.read_line(&mut first).unwrap();
    assert_eq!(first, "Enter two integers:\n");
    let mut input = child.stdin.take().unwrap();
    input.write_all(b"20 22\n").unwrap();
    drop(input);
    let mut rest = String::new();
    out.read_to_string(&mut rest).unwrap();
    assert_eq!(rest, "Sum:\n42\n");
    assert!(child.wait().unwrap().success());
}
#[test]
fn compile_and_run_bytecode_with_standard_io() {
    let temp = Temp::new();
    let binary = temp.0.join("program.atbc");
    let out = cli()
        .args(["--compile", "examples/sum.atomc"])
        .arg(&binary)
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let mut child = cli()
        .arg("--run-bytecode")
        .arg(&binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"-2 12\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"Enter two integers:\nSum:\n10\n");
    assert!(!cli()
        .args(["--compile", "examples/sum.atomc"])
        .arg(&binary)
        .output()
        .unwrap()
        .status
        .success());
}
#[test]
fn rejects_bounds_invalid_source_and_host_imports() {
    let temp = Temp::new();
    for source in [
        "int main(){int a[2];a[2]=1;return 0;}",
        "int main(){int a[2];a[-1]=1;return 0;}",
        "int main(){getenv(\"ATOMC_PRIVATE_TEST\");return 0;}",
        "int main(){fopen(\"secret\");return 0;}",
        "int main(){system(\"touch forbidden\");return 0;}",
        "int main( {",
        "int main(){return 1/0;}",
    ] {
        let path = temp.source(source);
        let out = cli()
            .arg(path)
            .env("ATOMC_PRIVATE_TEST", "secret sentinel")
            .output()
            .unwrap();
        assert!(!out.status.success(), "{source}");
        assert!(!String::from_utf8_lossy(&out.stderr).contains("secret sentinel"));
        assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
    }
    assert!(!temp.0.join("forbidden").exists());
}
#[test]
fn bounded_strings_and_io_errors_are_reported() {
    let temp = Temp::new();
    let path = temp.source("import io; int main(){char s[4];get_s(s);put_s(s);return 0;}");
    for (input, ok, expected) in [("abc\n", true, "abc\n"), ("abcd\n", false, "")] {
        let mut child = cli()
            .arg(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.success(), ok);
        assert_eq!(out.stdout, expected.as_bytes());
    }
    let path = temp.source("import io; int main(){put_i(get_i());return 0;}");
    let out = cli().arg(path).stdin(Stdio::null()).output().unwrap();
    assert!(!out.status.success());
}
#[test]
fn malformed_bytecode_and_nonterminating_program_fail() {
    let temp = Temp::new();
    let path = temp.0.join("invalid.atbc");
    fs::write(&path, b"bad file").unwrap();
    assert!(!cli()
        .arg("--run-bytecode")
        .arg(path)
        .output()
        .unwrap()
        .status
        .success());
    let path = temp.source("int main(){while(1){}return 0;}");
    let out = cli().arg(path).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("resource limit"));
}

#[test]
fn io_requires_an_explicit_library_import() {
    let temp = Temp::new();
    let path = temp.source("int main(){put_i(1);return 0;}");
    let out = cli().arg(path).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("undefined symbol: put_i"));
    let path = temp.source("import unknown;int main(){return 0;}");
    let out = cli().arg(path).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown built-in library"));
}

#[test]
fn optional_math_library_runs_from_source_and_bytecode() {
    let temp = Temp::new();
    let binary = temp.0.join("math.atbc");
    let out = cli()
        .args(["--compile", "examples/math.atomc"])
        .arg(&binary)
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    for args in [
        vec!["examples/math.atomc".to_string()],
        vec!["--run-bytecode".into(), binary.to_str().unwrap().into()],
    ] {
        let mut child = cli()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"9\n").unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{:?}", out);
        assert_eq!(
            out.stdout,
            b"Enter a nonnegative number:\nSquare root:\n3\nSquare:\n81\n"
        );
    }
    let path = temp.source("import math;int main(){sqrt(-1);return 0;}");
    let out = cli().arg(path).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("math domain or range error"));
}

#[test]
fn custom_imports_resolve_relative_to_the_importer_and_bytecode_is_self_contained() {
    let temp = Temp::new();
    let project = temp.0.join("project");
    let elsewhere = temp.0.join("elsewhere");
    fs::create_dir_all(project.join("lib")).unwrap();
    fs::create_dir(&elsewhere).unwrap();
    let entry = project.join("main.atomc");
    fs::write(
        project.join("common.atomc"),
        "int twice(int n){return n*2;}",
    )
    .unwrap();
    fs::write(
        project.join("lib/a.atomc"),
        "import \"../common.atomc\";int a(int n){return twice(n);}",
    )
    .unwrap();
    fs::write(
        project.join("b.atomc"),
        "import \"common.atomc\";int b(){return twice(1);}",
    )
    .unwrap();
    fs::write(&entry,"import io;import \"lib/a.atomc\";import \"b.atomc\";int main(){put_i(a(get_i())+b());return 0;}").unwrap();
    let mut child = cli()
        .arg(&entry)
        .current_dir(&elsewhere)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"20\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(out.stdout, b"42\n");
    let binary = temp.0.join("program.atbc");
    let second = temp.0.join("second.atbc");
    for output in [&binary, &second] {
        let out = cli()
            .arg("--compile")
            .arg(&entry)
            .arg(output)
            .current_dir(&elsewhere)
            .output()
            .unwrap();
        assert!(out.status.success(), "{:?}", out);
    }
    assert_eq!(fs::read(&binary).unwrap(), fs::read(second).unwrap());
    fs::remove_dir_all(project).unwrap();
    let mut child = cli()
        .arg("--run-bytecode")
        .arg(binary)
        .current_dir(&elsewhere)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"20\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(out.stdout, b"42\n");
}

#[test]
fn custom_import_errors_report_the_importing_file_or_dependency_location() {
    let temp = Temp::new();
    let entry = temp.source("import \"library.atomc\";int main(){return 0;}");
    let library = temp.0.join("library.atomc");
    fs::write(&library, "int f(){\n    return missing;\n}").unwrap();
    let out = cli().arg(&entry).output().unwrap();
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.contains("library.atomc"), "{error}");
    assert!(error.contains("Line: 2"), "{error}");
    assert!(error.contains("undefined symbol: missing"), "{error}");
    fs::write(&library, "int x").unwrap();
    let out = cli().arg(&entry).output().unwrap();
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.contains("library.atomc"));
    assert!(error.contains("end of source module"));
    fs::remove_file(library).unwrap();
    let out = cli().arg(&entry).output().unwrap();
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.contains("program.atomc:1:8"), "{error}");
    assert!(error.contains("could not resolve import"));
}

#[test]
fn circular_conflicting_and_entry_defining_imports_fail_cleanly() {
    let temp = Temp::new();
    let entry = temp.source("import \"a.atomc\";int main(){return 0;}");
    fs::write(temp.0.join("a.atomc"), "import \"b.atomc\";").unwrap();
    fs::write(temp.0.join("b.atomc"), "import \"a.atomc\";").unwrap();
    let out = cli().arg(&entry).output().unwrap();
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.contains("circular source import"));
    assert!(error.contains("a.atomc ->"));
    assert!(error.contains("b.atomc ->"));
    fs::write(temp.0.join("a.atomc"), "int main(){return 0;}").unwrap();
    let out = cli().arg(&entry).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("only the entry file"));
    fs::write(temp.0.join("a.atomc"), "int x;").unwrap();
    temp.source("import \"a.atomc\";int x;int main(){return 0;}");
    let out = cli().arg(&entry).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("symbol redefinition: x"));
}

#[test]
fn custom_imports_cannot_resolve_outside_the_entry_directory() {
    let temp = Temp::new();
    let project = temp.0.join("project");
    fs::create_dir(&project).unwrap();
    let outside = temp.0.join("outside.atomc");
    fs::write(&outside, "int external;").unwrap();
    let entry = project.join("main.atomc");
    fs::write(&entry, "import \"../outside.atomc\";int main(){return 0;}").unwrap();
    let out = cli().arg(&entry).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("outside entry directory"));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, project.join("link.atomc")).unwrap();
        fs::write(&entry, "import \"link.atomc\";int main(){return 0;}").unwrap();
        let out = cli().arg(&entry).output().unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("outside entry directory"));
    }
}

#[test]
fn multi_file_example_runs_with_standard_io() {
    let mut child = cli()
        .arg("examples/modules/main.atomc")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"2\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        out.stdout,
        b"Enter a circle radius:\nArea:\n12.566370614359172\nCalculations:\n1\n"
    );
}
