//! Tool results, with lazily materialised output (SPEC §9.2).

use std::borrow::Cow;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use super::INLINE_OUTPUT_LIMIT;

/// Where the bytes of a transcript live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteSource {
    /// A transcript file on disk.
    File(Arc<PathBuf>),
    /// An in-memory transcript (used by tests).
    Memory(Arc<[u8]>),
}

impl ByteSource {
    /// Reads the bytes in `range` (absolute offsets into the transcript).
    pub fn read(&self, range: Range<u64>) -> io::Result<Vec<u8>> {
        let out_of_range =
            || io::Error::new(io::ErrorKind::UnexpectedEof, "byte range out of range");
        match self {
            ByteSource::Memory(bytes) => {
                let start = usize::try_from(range.start).map_err(|_| out_of_range())?;
                let end = usize::try_from(range.end).map_err(|_| out_of_range())?;
                bytes
                    .get(start..end)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(out_of_range)
            }
            ByteSource::File(path) => {
                let len = range
                    .end
                    .checked_sub(range.start)
                    .ok_or_else(out_of_range)?;
                let len = usize::try_from(len).map_err(|_| out_of_range())?;
                let mut file = File::open(path.as_path())?;
                file.seek(SeekFrom::Start(range.start))?;
                let mut buf = vec![0; len];
                file.read_exact(&mut buf)?;
                Ok(buf)
            }
        }
    }
}

/// How to pull a tool result's text out of a transcript line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineFormat {
    /// A Claude JSONL line.
    Claude,
}

impl LineFormat {
    fn extract(self, line: &[u8], call_id: &str) -> Option<String> {
        match self {
            LineFormat::Claude => crate::harness::claude::extract_result_text(line, call_id),
        }
    }
}

/// A reference to the transcript line a large result came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSource {
    /// The transcript.
    pub bytes: ByteSource,
    /// Byte range of the JSONL line (without its newline).
    pub range: Range<u64>,
    /// How to parse the line.
    pub format: LineFormat,
    /// The tool-use id naming the result block inside the line.
    pub call_id: String,
}

/// Why a lazy output could not be materialised.
#[derive(Debug)]
pub enum ToolOutputError {
    /// The transcript could not be read.
    Io(io::Error),
    /// The line no longer holds a result for the call, or it changed size.
    Changed,
}

impl fmt::Display for ToolOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolOutputError::Io(err) => write!(f, "cannot read tool output: {err}"),
            ToolOutputError::Changed => f.write_str("tool output no longer matches the transcript"),
        }
    }
}

impl std::error::Error for ToolOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToolOutputError::Io(err) => Some(err),
            ToolOutputError::Changed => None,
        }
    }
}

impl From<io::Error> for ToolOutputError {
    fn from(err: io::Error) -> Self {
        ToolOutputError::Io(err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Repr {
    Inline {
        text: String,
        line_count: usize,
    },
    Lazy {
        line_count: usize,
        byte_len: usize,
        source: OutputSource,
    },
}

/// The output of a tool call: inline when small, a line count plus a source
/// reference when larger than [`INLINE_OUTPUT_LIMIT`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput(Repr);

/// Counts lines the way [`ToolOutput::line_count`] reports them.
pub(crate) fn count_lines(text: &str) -> usize {
    text.lines().count()
}

impl ToolOutput {
    /// Stores `text` inline.
    pub fn inline(text: String) -> Self {
        let line_count = count_lines(&text);
        Self(Repr::Inline { text, line_count })
    }

    /// Stores only the statistics and the source of the text.
    pub fn lazy(line_count: usize, byte_len: usize, source: OutputSource) -> Self {
        Self(Repr::Lazy {
            line_count,
            byte_len,
            source,
        })
    }

    /// Stores `text` inline when it is at most [`INLINE_OUTPUT_LIMIT`] bytes,
    /// otherwise drops it and keeps `source()` for later re-reading.
    pub fn from_text(text: String, source: impl FnOnce() -> OutputSource) -> Self {
        if text.len() <= INLINE_OUTPUT_LIMIT {
            Self::inline(text)
        } else {
            Self::lazy(count_lines(&text), text.len(), source())
        }
    }

    /// Number of lines, available without materialising.
    pub fn line_count(&self) -> usize {
        match &self.0 {
            Repr::Inline { line_count, .. } | Repr::Lazy { line_count, .. } => *line_count,
        }
    }

    /// Length of the text in bytes, available without materialising.
    pub fn byte_len(&self) -> usize {
        match &self.0 {
            Repr::Inline { text, .. } => text.len(),
            Repr::Lazy { byte_len, .. } => *byte_len,
        }
    }

    /// Whether the text is stored by reference rather than inline.
    pub fn is_lazy(&self) -> bool {
        matches!(self.0, Repr::Lazy { .. })
    }

    /// The full text, re-read from the transcript if it is stored lazily.
    pub fn text(&self) -> Result<Cow<'_, str>, ToolOutputError> {
        match &self.0 {
            Repr::Inline { text, .. } => Ok(Cow::Borrowed(text)),
            Repr::Lazy {
                byte_len, source, ..
            } => {
                let line = source.bytes.read(source.range.clone())?;
                let text = source
                    .format
                    .extract(&line, &source.call_id)
                    .ok_or(ToolOutputError::Changed)?;
                if text.len() != *byte_len {
                    return Err(ToolOutputError::Changed);
                }
                Ok(Cow::Owned(text))
            }
        }
    }
}

/// The result of a tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    /// The output text.
    pub output: ToolOutput,
    /// Whether the tool reported an error.
    pub is_error: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(bytes: &'static [u8], range: Range<u64>, call_id: &str) -> OutputSource {
        OutputSource {
            bytes: ByteSource::Memory(Arc::from(bytes)),
            range,
            format: LineFormat::Claude,
            call_id: call_id.to_owned(),
        }
    }

    #[test]
    fn small_text_is_inline() {
        let out = ToolOutput::from_text("a\nb\n".into(), || unreachable!());
        assert!(!out.is_lazy());
        assert_eq!((out.line_count(), out.byte_len()), (2, 4));
        assert_eq!(out.text().unwrap(), "a\nb\n");
    }

    #[test]
    fn empty_text_has_no_lines() {
        let out = ToolOutput::inline(String::new());
        assert_eq!((out.line_count(), out.byte_len()), (0, 0));
    }

    #[test]
    fn large_text_is_replaced_by_its_source() {
        let line =
            br#"{"message":{"content":[{"type":"tool_result","tool_use_id":"t","content":"hi"}]}}"#;
        let text = "z\n".repeat(3000);
        let out = ToolOutput::from_text(text.clone(), || source(line, 0..line.len() as u64, "t"));
        assert!(out.is_lazy());
        assert_eq!((out.line_count(), out.byte_len()), (3000, text.len()));
        // The source line holds different text, so re-reading detects a mismatch.
        assert!(matches!(out.text(), Err(ToolOutputError::Changed)));
    }

    #[test]
    fn out_of_range_source_is_an_io_error() {
        let out = ToolOutput::lazy(1, 2, source(b"{}", 0..99, "t"));
        assert!(matches!(out.text(), Err(ToolOutputError::Io(_))));
    }

    #[test]
    fn missing_call_id_is_reported_as_changed() {
        let line = br#"{"message":{"content":[{"type":"tool_result","tool_use_id":"other","content":"hi"}]}}"#;
        let out = ToolOutput::lazy(1, 2, source(line, 0..line.len() as u64, "t"));
        assert!(matches!(out.text(), Err(ToolOutputError::Changed)));
    }

    #[test]
    fn lazy_text_is_read_from_its_range_among_other_lines() {
        let line =
            br#"{"message":{"content":[{"type":"tool_result","tool_use_id":"t","content":"hi"}]}}"#;
        let mut all = b"prefix line\n".to_vec();
        let start = all.len() as u64;
        all.extend_from_slice(line);
        all.extend_from_slice(b"\nsuffix\n");
        let all: &'static [u8] = Box::leak(all.into_boxed_slice());
        let out = ToolOutput::lazy(1, 2, source(all, start..start + line.len() as u64, "t"));
        assert_eq!(out.text().unwrap(), "hi");
    }
}
