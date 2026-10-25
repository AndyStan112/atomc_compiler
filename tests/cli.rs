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
    let path = temp.source("int main(){char s[4];get_s(s);put_s(s);return 0;}");
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
    let path = temp.source("int main(){put_i(get_i());return 0;}");
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
