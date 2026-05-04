use std::path::Path;

use cell_sheet_core::io::csv as csv_io;

/// File/container formats supported by the TUI and headless CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Csv,
    Tsv,
    Cell,
}

impl FileFormat {
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "tsv" => FileFormat::Tsv,
            "cell" => FileFormat::Cell,
            _ => FileFormat::Csv,
        }
    }

    pub fn from_stdin_bytes(data: &[u8]) -> Self {
        if data.starts_with(CELL_FORMAT_MAGIC) {
            FileFormat::Cell
        } else {
            FileFormat::Csv
        }
    }

    pub fn canonical_delimiter(self) -> Option<u8> {
        match self {
            FileFormat::Csv => Some(b','),
            FileFormat::Tsv => Some(b'\t'),
            FileFormat::Cell => None,
        }
    }

    pub fn resolve_path_delimiter(
        self,
        path: &Path,
        explicit: Option<u8>,
    ) -> Result<u8, Box<dyn std::error::Error>> {
        if let Some(d) = explicit {
            return Ok(d);
        }
        match self {
            FileFormat::Tsv => Ok(b'\t'),
            FileFormat::Cell => Ok(b','), // unused: .cell files do not use a delimiter
            FileFormat::Csv => {
                use std::io::Read as _;

                let mut buf = vec![0u8; 4096];
                let mut file = std::fs::File::open(path)?;
                let n = file.read(&mut buf)?;
                Ok(csv_io::sniff_delimiter(&buf[..n]))
            }
        }
    }

    pub fn resolve_data_delimiter(self, data: &[u8], explicit: Option<u8>) -> u8 {
        if let Some(d) = explicit {
            return d;
        }
        match self {
            FileFormat::Tsv => b'\t',
            FileFormat::Cell => b',', // unused: .cell files do not use a delimiter
            FileFormat::Csv => csv_io::sniff_delimiter(data),
        }
    }

    pub fn resolve_stdin_delimiter(
        self,
        data: &[u8],
        explicit: Option<u8>,
    ) -> Result<u8, &'static str> {
        if matches!(self, FileFormat::Cell) && explicit.is_some() {
            return Err("--delimiter has no effect on .cell-format input piped to stdin");
        }
        Ok(self.resolve_data_delimiter(data, explicit))
    }
}

/// Magic header for the native `.cell` text format. Both the existing writer
/// and reader anchor on `# cell v` as the first line, so detecting it on stdin
/// is unambiguous.
const CELL_FORMAT_MAGIC: &[u8] = b"# cell v";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_path_detects_known_extensions() {
        assert_eq!(
            FileFormat::from_path(Path::new("data.csv")),
            FileFormat::Csv
        );
        assert_eq!(
            FileFormat::from_path(Path::new("data.tsv")),
            FileFormat::Tsv
        );
        assert_eq!(
            FileFormat::from_path(Path::new("data.cell")),
            FileFormat::Cell
        );
        assert_eq!(
            FileFormat::from_path(Path::new("data.psv")),
            FileFormat::Csv
        );
    }

    #[test]
    fn stdin_format_detects_cell_magic() {
        assert_eq!(
            FileFormat::from_stdin_bytes(b"# cell v1\n"),
            FileFormat::Cell
        );
        assert_eq!(FileFormat::from_stdin_bytes(b"a,b\n"), FileFormat::Csv);
    }

    #[test]
    fn stdin_cell_rejects_explicit_delimiter() {
        assert!(FileFormat::Cell
            .resolve_stdin_delimiter(b"# cell v1\n", Some(b'|'))
            .is_err());
    }

    #[test]
    fn path_delimiter_uses_tsv_default() {
        let delimiter = FileFormat::Tsv
            .resolve_path_delimiter(Path::new("unused.tsv"), None)
            .expect("tsv delimiter should not require reading the file");
        assert_eq!(delimiter, b'\t');
    }
}
