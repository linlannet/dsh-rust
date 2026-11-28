//! 文件读取沙箱。

use std::fs;
use std::path::{Component, Path, PathBuf};

use dsh_types::{Error, ErrorCode};

/// 只读文件沙箱：路径必须落在根目录内，且不超过大小上限。
#[derive(Debug, Clone)]
pub struct ReadFileSandbox {
    root: PathBuf,
    max_bytes: usize,
}

impl ReadFileSandbox {
    /// 构造沙箱；根目录必须存在。
    pub fn new(root: impl Into<PathBuf>, max_bytes: usize) -> Result<Self, Error> {
        let root = root.into();
        let root = root.canonicalize().map_err(|error| {
            Error::new(
                ErrorCode::InvalidRequest,
                format!("workspace root is not accessible: {error}"),
            )
        })?;
        if !root.is_dir() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "workspace root must be a directory",
            ));
        }
        Ok(Self { root, max_bytes })
    }

    /// 根目录。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 大小上限。
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// 解析并校验输入路径，返回相对路径与绝对路径。
    pub fn resolve(&self, input: &str) -> Result<(String, PathBuf), Error> {
        let raw = Path::new(input);
        if raw.is_absolute() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "read_file requires a relative path",
            ));
        }
        if raw
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "read_file rejects parent directory traversal",
            ));
        }
        let joined = self.root.join(raw);
        let canonical = joined.canonicalize().map_err(|error| {
            let code = if error.kind() == std::io::ErrorKind::NotFound {
                ErrorCode::NotFound
            } else {
                ErrorCode::InvalidRequest
            };
            Error::new(code, format!("cannot resolve path: {error}"))
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "path escapes workspace root",
            ));
        }
        let relative = canonical
            .strip_prefix(&self.root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        Ok((relative, canonical))
    }

    /// 读取 UTF-8 文本。
    pub fn read(&self, input: &str) -> Result<(String, String, usize), Error> {
        let (relative, path) = self.resolve(input)?;
        let metadata = fs::metadata(&path).map_err(|error| {
            Error::new(ErrorCode::NotFound, format!("cannot stat file: {error}"))
        })?;
        if !metadata.is_file() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "read_file only supports regular files",
            ));
        }
        if metadata.len() > self.max_bytes as u64 {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                format!(
                    "file exceeds read limit: {} > {}",
                    metadata.len(),
                    self.max_bytes
                ),
            ));
        }
        let bytes = fs::read(&path).map_err(|error| {
            Error::new(ErrorCode::NotFound, format!("cannot read file: {error}"))
        })?;
        let size = bytes.len();
        let content = String::from_utf8(bytes)
            .map_err(|_| Error::new(ErrorCode::InvalidRequest, "file is not valid UTF-8 text"))?;
        Ok((relative, content, size))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;

    fn temp_root() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!(
            "dsh-sandbox-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn reads_file_inside_root() {
        let root = temp_root();
        fs::write(root.join("hello.txt"), "hello").expect("write");
        let sandbox = ReadFileSandbox::new(&root, 1024).expect("sandbox");
        let (relative, content, bytes) = sandbox.read("hello.txt").expect("read");
        assert_eq!(relative, "hello.txt");
        assert_eq!(content, "hello");
        assert_eq!(bytes, 5);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_parent_traversal() {
        let root = temp_root();
        let sandbox = ReadFileSandbox::new(&root, 1024).expect("sandbox");
        let error = sandbox.read("../secret.txt").expect_err("must reject");
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_absolute_path() {
        let root = temp_root();
        let sandbox = ReadFileSandbox::new(&root, 1024).expect("sandbox");
        let error = sandbox.read("/etc/passwd").expect_err("must reject");
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_oversized_file() {
        let root = temp_root();
        let mut file = fs::File::create(root.join("big.txt")).expect("create");
        file.write_all(&[b'a'; 32]).expect("write");
        let sandbox = ReadFileSandbox::new(&root, 8).expect("sandbox");
        let error = sandbox.read("big.txt").expect_err("must reject");
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_non_utf8() {
        let root = temp_root();
        fs::write(root.join("bin.dat"), [0xff, 0xfe, 0x00]).expect("write");
        let sandbox = ReadFileSandbox::new(&root, 1024).expect("sandbox");
        let error = sandbox.read("bin.dat").expect_err("must reject");
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_missing_file() {
        let root = temp_root();
        let sandbox = ReadFileSandbox::new(&root, 1024).expect("sandbox");
        let error = sandbox.read("missing.txt").expect_err("must fail");
        assert_eq!(error.code(), ErrorCode::NotFound);
        let _ = fs::remove_dir_all(root);
    }
}
