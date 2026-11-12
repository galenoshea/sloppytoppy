//! Re-parse a log only when its size or modification time changed.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::{SessionUsage, UsageError};

#[derive(Debug)]
struct Entry {
    modified: SystemTime,
    len: u64,
    usage: SessionUsage,
}

/// Memoises parsed sessions keyed by file path, invalidated by `(mtime, len)`.
#[derive(Debug, Default)]
pub(crate) struct FileCache {
    entries: HashMap<PathBuf, Entry>,
}

impl FileCache {
    pub(crate) fn get_or_parse(
        &mut self,
        path: &Path,
        parse: impl FnOnce(BufReader<File>) -> io::Result<SessionUsage>,
    ) -> Result<SessionUsage, UsageError> {
        let meta = fs::metadata(path).map_err(|e| UsageError::io(path, e))?;
        let modified = meta.modified().map_err(|e| UsageError::io(path, e))?;
        let len = meta.len();

        if let Some(hit) = self.entries.get(path)
            && hit.modified == modified
            && hit.len == len
        {
            return Ok(hit.usage.clone());
        }

        let file = File::open(path).map_err(|e| UsageError::io(path, e))?;
        let usage = parse(BufReader::new(file)).map_err(|e| UsageError::io(path, e))?;
        self.entries.insert(
            path.to_path_buf(),
            Entry {
                modified,
                len,
                usage: usage.clone(),
            },
        );
        Ok(usage)
    }
}
