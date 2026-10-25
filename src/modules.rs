//! Compile-time source loading. No module paths or file handles enter the VM.
use crate::builtins::Library;
use crate::lexer::{Lexer, Token, TokenCode as T};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_FILES: usize = 128;
pub const MAX_IMPORT_DEPTH: usize = 32;
type Result<T> = std::result::Result<T, String>;

pub struct SourceFile {
    pub name: String,
    pub text: String,
    /// Token index immediately after the import header.
    pub body_start: usize,
}
impl SourceFile {
    pub fn tokens(&self) -> Result<Vec<Token<'_>>> {
        lex(&self.text, &self.name)
    }
}

pub fn lex<'a>(text: &'a str, name: &'a str) -> Result<Vec<Token<'a>>> {
    if text.contains('\0') {
        return Err(format!("{name}: NUL bytes are not allowed in source files"));
    }
    crate::diagnostic::catch(|| {
        let mut lexer = if name == "<source>" {
            Lexer::new(text)
        } else {
            Lexer::with_source(text, name)
        };
        Ok(lexer.get_tokens().clone())
    })
    .map_err(|e| format!("{name}: {e}"))
}

enum Import {
    Builtin,
    Source(String, String),
}
fn header(tokens: &[Token<'_>]) -> Result<(usize, Vec<Import>)> {
    let mut pos = 0;
    let mut imports = Vec::new();
    while tokens[pos].code == T::IMPORT {
        let name = &tokens[pos + 1];
        let import = match &name.code {
            T::ID(name_text) => {
                Library::imported(name_text).ok_or_else(|| {
                    format!("{}: unknown built-in library: {name_text}", name.location())
                })?;
                Import::Builtin
            }
            T::CT_STRING(path) => {
                if path.is_empty()
                    || path.contains('\\')
                    || path.contains('\0')
                    || Path::new(path).is_absolute()
                {
                    return Err(format!("{}: source import requires a nonempty relative path using '/' without escapes",name.location()));
                }
                if Path::new(path).extension().is_none_or(|ext| ext != "atomc") {
                    return Err(format!(
                        "{}: source import must name an .atomc file",
                        name.location()
                    ));
                }
                Import::Source(path.to_string(), name.location())
            }
            _ => {
                return Err(format!(
                    "{}: expected a built-in name or quoted source path after import",
                    name.location()
                ))
            }
        };
        let semicolon = tokens
            .get(pos + 2)
            .ok_or_else(|| format!("{}: expected ';' after import", name.location()))?;
        if semicolon.code != T::SEMICOLON {
            return Err(format!(
                "{}: expected ';' after import",
                semicolon.location()
            ));
        }
        imports.push(import);
        pos += 3;
    }
    if let Some(late) = tokens[pos..].iter().find(|token| token.code == T::IMPORT) {
        return Err(format!(
            "{}: imports must precede all declarations",
            late.location()
        ));
    }
    Ok((pos, imports))
}

/// Return each canonical file exactly once, dependencies first, entry last.
pub fn load(entry: &Path) -> Result<Vec<SourceFile>> {
    let entry = fs::canonicalize(entry)
        .map_err(|e| format!("{}: could not resolve source: {e}", entry.display()))?;
    let root = entry
        .parent()
        .ok_or("entry file has no parent directory")?
        .to_path_buf();
    let mut loader = Loader {
        root,
        active: Vec::new(),
        done: BTreeSet::new(),
        files: Vec::new(),
        bytes: 0,
    };
    loader.visit(entry)?;
    Ok(loader.files)
}
struct Loader {
    root: PathBuf,
    active: Vec<PathBuf>,
    done: BTreeSet<PathBuf>,
    files: Vec<SourceFile>,
    bytes: usize,
}
impl Loader {
    fn visit(&mut self, path: PathBuf) -> Result<()> {
        if let Some(start) = self.active.iter().position(|p| p == &path) {
            let mut chain: Vec<_> = self.active[start..]
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            chain.push(path.display().to_string());
            return Err(format!("circular source import: {}", chain.join(" -> ")));
        }
        if self.done.contains(&path) {
            return Ok(());
        }
        if self.active.len() >= MAX_IMPORT_DEPTH {
            return Err(format!(
                "{}: import depth exceeds {MAX_IMPORT_DEPTH}",
                path.display()
            ));
        }
        if self.files.len() + self.active.len() >= MAX_FILES {
            return Err(format!("source file count exceeds {MAX_FILES}"));
        }
        // Check before opening so directories/devices/FIFOs are not treated as input streams.
        let metadata = fs::metadata(&path)
            .map_err(|e| format!("{}: could not inspect source: {e}", path.display()))?;
        if !metadata.is_file() {
            return Err(format!("{}: source must be a regular file", path.display()));
        }
        let name = path
            .to_str()
            .ok_or("source paths must be UTF-8")?
            .to_string();
        let file = File::open(&path).map_err(|e| format!("{name}: could not open source: {e}"))?;
        if !file
            .metadata()
            .map_err(|e| format!("{name}: {e}"))?
            .is_file()
        {
            return Err(format!("{name}: source must be a regular file"));
        }
        let remaining = MAX_SOURCE_BYTES - self.bytes;
        let mut bytes = Vec::new();
        file.take(remaining as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{name}: could not read source: {e}"))?;
        if bytes.len() > remaining {
            return Err(format!("{name}: combined source exceeds 1 MiB"));
        }
        self.bytes += bytes.len();
        let text = String::from_utf8(bytes).map_err(|_| format!("{name}: source must be UTF-8"))?;
        let tokens = lex(&text, &name)?;
        let (body_start, imports) = header(&tokens)?;
        self.active.push(path.clone());
        for import in imports {
            if let Import::Source(relative, location) = import {
                let candidate = path.parent().unwrap().join(relative);
                let canonical = fs::canonicalize(&candidate).map_err(|e| {
                    format!(
                        "{location}: could not resolve import {}: {e}",
                        candidate.display()
                    )
                })?;
                if !canonical.starts_with(&self.root) {
                    return Err(format!(
                        "{location}: import resolves outside entry directory: {}",
                        canonical.display()
                    ));
                }
                self.visit(canonical)
                    .map_err(|e| format!("{location}: imported from here\n{e}"))?;
            }
        }
        self.active.pop();
        self.done.insert(path);
        self.files.push(SourceFile {
            name,
            text,
            body_start,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Tree(PathBuf);
    impl Tree {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "atomc-modules-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn write(&self, path: &str, text: &str) -> PathBuf {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, text).unwrap();
            path
        }
    }
    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn error(entry: &Path) -> String {
        load(entry).err().expect("expected loader error")
    }
    #[test]
    fn dependency_order_is_relative_deduplicated_and_deterministic() {
        let tree = Tree::new();
        tree.write("common.atomc", "int shared;");
        tree.write("lib/a.atomc", "import \"../common.atomc\";int a;");
        tree.write("b.atomc", "import \"./common.atomc\";int b;");
        let entry = tree.write("main.atomc", "import io;import \"lib/a.atomc\";import \"b.atomc\";import \"lib/../common.atomc\";int main(){return 0;}");
        let files = load(&entry).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|s| Path::new(&s.name).file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["common.atomc", "a.atomc", "b.atomc", "main.atomc"]);
        assert_eq!(files.last().unwrap().body_start, 12);
        assert_eq!(
            load(&entry)
                .unwrap()
                .iter()
                .map(|f| &f.name)
                .collect::<Vec<_>>(),
            files.iter().map(|f| &f.name).collect::<Vec<_>>()
        );
        assert!(files[0].tokens().unwrap()[0]
            .source
            .ends_with("common.atomc"));
    }
    #[test]
    fn cycles_missing_files_and_malformed_headers_have_locations() {
        let tree = Tree::new();
        tree.write("a.atomc", "import \"b.atomc\";");
        tree.write("b.atomc", "import \"a.atomc\";");
        let entry = tree.write("main.atomc", "import \"a.atomc\";int main(){return 0;}");
        let message = error(&entry);
        assert!(message.contains("circular source import"));
        assert!(message.contains("a.atomc ->"));
        assert!(message.contains("main.atomc:1:8"));
        for (text, expected) in [
            ("import \"missing.atomc\";", "could not resolve import"),
            ("import unknown;", "unknown built-in library"),
            ("import", "expected a built-in name"),
            ("import io", "expected ';'"),
            ("import \"a.atomc\"", "expected ';'"),
            ("import \"a.txt\";", "must name an .atomc file"),
            ("import \"\";", "nonempty relative path"),
            ("int x;import io;", "imports must precede"),
            ("int x;\0", "NUL bytes"),
        ] {
            tree.write("main.atomc", text);
            let message = error(&entry);
            assert!(message.contains(expected), "{message}");
            assert!(message.contains("main.atomc"), "{message}");
        }
    }
    #[test]
    fn rejects_absolute_paths_root_escapes_and_nonregular_files() {
        let tree = Tree::new();
        let outside = tree.write("outside.atomc", "int secret;");
        let entry = tree.write("project/main.atomc", "import \"../outside.atomc\";");
        assert!(error(&entry).contains("outside entry directory"));
        tree.write(
            "project/main.atomc",
            &format!("import \"{}\";", outside.display()),
        );
        assert!(error(&entry).contains("relative path"));
        fs::create_dir(tree.0.join("project/directory.atomc")).unwrap();
        tree.write("project/main.atomc", "import \"directory.atomc\";");
        assert!(error(&entry).contains("regular file"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, tree.0.join("project/link.atomc")).unwrap();
            tree.write("project/main.atomc", "import \"link.atomc\";");
            assert!(error(&entry).contains("outside entry directory"));
        }
    }
    #[test]
    fn aggregate_size_file_count_and_depth_are_bounded() {
        let tree = Tree::new();
        tree.write("large.atomc", &" ".repeat(MAX_SOURCE_BYTES));
        let entry = tree.write("main.atomc", "import \"large.atomc\";");
        assert!(error(&entry).contains("combined source exceeds"));
        let mut imports = String::new();
        for i in 0..MAX_FILES {
            tree.write(&format!("f{i}.atomc"), "");
            imports.push_str(&format!("import \"f{i}.atomc\";"));
        }
        tree.write("main.atomc", &imports);
        assert!(error(&entry).contains("file count exceeds"));
        for i in 0..MAX_IMPORT_DEPTH {
            tree.write(
                &format!("f{i}.atomc"),
                &format!("import \"f{}.atomc\";", i + 1),
            );
        }
        tree.write("main.atomc", "import \"f0.atomc\";");
        assert!(error(&entry).contains("import depth exceeds"));
    }
}
