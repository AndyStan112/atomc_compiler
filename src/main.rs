#![forbid(unsafe_code)]

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};

mod analyser;
mod builtins;
mod bytecode;
mod compiler;
mod layout;
mod lexer;
mod vm;

const USAGE: &str = "Usage: AtomC [SOURCE]\n       AtomC --compile SOURCE OUTPUT.atbc\n       AtomC --run-bytecode PROGRAM.atbc\nGuest I/O uses stdin/stdout. Default source: data/main.atomc";

fn read_file(path: &str, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("could not open {path}: {e}"))?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err(format!("{path} exceeds input size limit"));
    }
    Ok(bytes)
}
fn source(path: &str) -> Result<compiler::Program, String> {
    let text =
        String::from_utf8(read_file(path, 1024 * 1024)?).map_err(|_| "source must be UTF-8")?;
    compiler::compile(&text)
}
fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let program = match args.as_slice() {
        [] => source("data/main.atomc")?,
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{USAGE}");
            return Ok(());
        }
        [path] if !path.starts_with('-') => source(path)?,
        [flag, input, output] if flag == "--compile" => {
            let bytes = bytecode::encode(&source(input)?)?;
            // Do not silently overwrite an existing artifact or source file.
            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(output)
                .map_err(|e| format!("could not create {output}: {e}"))?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            return Ok(());
        }
        [flag, input] if flag == "--run-bytecode" => {
            bytecode::decode(&read_file(input, bytecode::MAX_BYTECODE)?)?
        }
        _ => return Err(USAGE.into()),
    };
    let mut vm = program.into_vm("")?;
    vm.run_with_io(&mut io::stdin().lock(), &mut io::stdout().lock())
        .map_err(|e| e.to_string())
}
fn main() {
    // The inherited lexer/analyser use panic diagnostics. The compiler catches
    // those; keep the CLI diagnostic concise instead of printing a panic hook too.
    std::panic::set_hook(Box::new(|_| {}));
    if let Err(error) = run() {
        eprintln!("AtomC: {error}");
        std::process::exit(1);
    }
}
