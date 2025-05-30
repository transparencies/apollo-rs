//! Pretty-printable diagnostic reports for errors that reference GraphQL documents.
//!
//! # Usage
//! To use pretty-printing in custom errors, implement the [`ToCliReport`] trait.
//!
//! ```rust
//! use apollo_compiler::parser::SourceSpan;
//! use apollo_compiler::Schema;
//! use apollo_compiler::Name;
//! use apollo_compiler::diagnostic::CliReport;
//! use apollo_compiler::diagnostic::Diagnostic;
//! use apollo_compiler::diagnostic::ToCliReport;
//!
//! /// Error type for a small GraphQL schema linter.
//! #[derive(Debug, thiserror::Error)]
//! enum LintError {
//!     #[error("{name} should be PascalCase")]
//!     InvalidCase { name: Name },
//!     #[error("Missing @specifiedBy directive on scalar {name}")]
//!     NoSpecifiedBy {
//!         location: Option<SourceSpan>,
//!         name: Name,
//!     },
//! }
//!
//! impl ToCliReport for LintError {
//!     fn location(&self) -> Option<SourceSpan> {
//!         match self {
//!             LintError::InvalidCase { name } => name.location(),
//!             LintError::NoSpecifiedBy { location, .. } => *location,
//!         }
//!     }
//!
//!     fn report(&self, report: &mut CliReport<'_>) {
//!         match self {
//!             LintError::InvalidCase { name } => {
//!                 report.with_label_opt(name.location(), "should be PascalCase");
//!                 report.with_help(format!("Try using {}", to_pascal_case(name)));
//!             }
//!             LintError::NoSpecifiedBy { location, .. } => {
//!                 report.with_label_opt(*location, "scalar does not have a specification");
//!             }
//!         }
//!     }
//! }
//!
//! # fn to_pascal_case(name: &str) -> String { todo!() }
//! ```
//!
//! The [`Diagnostic`] type wraps errors that implement [`ToCliReport`] and provides
//! the pretty-printing functionality. [`ToCliReport::to_diagnostic`] returns a diagnostic
//! ready for formatting:
//!
//! ```rust
//! # use apollo_compiler::{parser::SourceSpan, Schema, diagnostic::{ToCliReport, CliReport}};
//! # #[derive(Debug, thiserror::Error)]
//! # #[error("")]
//! # struct LintError {}
//! # impl ToCliReport for LintError {
//! #     fn location(&self) -> Option<SourceSpan> { None }
//! #     fn report(&self, _report: &mut CliReport) {}
//! # }
//! fn print_errors(schema: &Schema, errors: &[LintError]) {
//!     for error in errors {
//!         // Debug-formatting uses colors.
//!         eprintln!("{:?}", error.to_diagnostic(&schema.sources));
//!     }
//! }
//! ```
use crate::parser::FileId;
use crate::parser::LineColumn;
use crate::parser::SourceFile;
use crate::parser::SourceMap;
use crate::parser::SourceSpan;
use crate::response::GraphQLError;
#[cfg(doc)]
use crate::ExecutableDocument;
#[cfg(doc)]
use crate::Schema;
use ariadne::ColorGenerator;
use ariadne::ReportKind;
use std::cell::Cell;
use std::fmt;
use std::io;
use std::ops::Range;
use std::sync::Arc;
use std::sync::OnceLock;

/// An error bundled together with a source map, for conversion either
/// to a pretty-printable CLI report or to a JSON-serializable GraphQL error.
///
/// Implements [`fmt::Debug`] _with_ ANSI colors enabled,
/// for printing panic messages of [`Result<_, Diagnostic<_>>::unwrap`][Result::unwrap].
///
/// Implements [`fmt::Display`] _without_ colors,
/// so [`.to_string()`][ToString] can be used in more varied contexts like unit tests.
pub struct Diagnostic<'s, T>
where
    T: ToCliReport,
{
    pub sources: &'s SourceMap,
    pub error: &'s T,
}

/// A diagnostic report that can be printed to a CLI with pretty colors and labeled lines of
/// GraphQL source code.
///
/// Custom errors can use this in their `Display` or `Debug` implementations to build a report and
/// then write it out with [`fmt`].
///
/// [`fmt`]: CliReport::fmt
pub struct CliReport<'s> {
    sources: &'s SourceMap,
    colors: ColorGenerator,
    report: ariadne::ReportBuilder<'static, AriadneSpan>,
}

/// Indicate when to use ANSI colors for printing.
#[derive(Debug, Clone, Copy)]
pub enum Color {
    /// Do not use colors.
    Never,
    /// Use colors if stderr is a terminal.
    StderrIsTerminal,
}

/// Conversion to [`CliReport`]
pub trait ToCliReport: fmt::Display {
    /// Return the main location for this error. May be `None` if a location doesn't make sense for
    /// the particular error.
    fn location(&self) -> Option<SourceSpan>;

    /// Fill in the report with source code labels.
    ///
    /// The main message is already set to the output of [`fmt::Display`].
    fn report(&self, report: &mut CliReport<'_>);

    fn to_report<'s>(&self, sources: &'s SourceMap, color: Color) -> CliReport<'s> {
        let mut report = CliReport::builder(sources, self.location(), color);
        report.with_message(self);
        self.report(&mut report);
        report
    }

    /// Bundle this error together with a source map into a [`Diagnostic`]
    ///
    /// The map normally comes from [`Schema::sources`] or [`ExecutableDocument::sources`].
    fn to_diagnostic<'s>(&'s self, sources: &'s SourceMap) -> Diagnostic<'s, Self>
    where
        Self: Sized,
    {
        Diagnostic {
            sources,
            error: self,
        }
    }
}

impl<T: ToCliReport> ToCliReport for &T {
    fn location(&self) -> Option<SourceSpan> {
        ToCliReport::location(*self)
    }

    fn report(&self, report: &mut CliReport) {
        ToCliReport::report(*self, report)
    }
}

/// An ariadne span type. We avoid implementing `ariadne::Span` for `SourceSpan`
/// so ariadne doesn't leak into the public API of apollo-compiler.
type AriadneSpan = (FileId, Range<usize>);

/// Translate a SourceSpan into an ariadne span type.
fn to_span(location: SourceSpan) -> Option<AriadneSpan> {
    let start = location.offset();
    let end = location.end_offset();
    Some((location.file_id, start..end))
}

/// Provide a [`std::io::Write`] API for a [`std::fmt::Formatter`].
struct WriteToFormatter<'a, 'b> {
    f: &'a mut fmt::Formatter<'b>,
}

impl io::Write for WriteToFormatter<'_, '_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = std::str::from_utf8(buf).map_err(|_| io::ErrorKind::Other)?;
        self.f.write_str(s).map_err(|_| io::ErrorKind::Other)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'s> CliReport<'s> {
    /// Returns a builder for creating diagnostic reports.
    ///
    /// Provide GraphQL source files and the main location for the diagnostic.
    /// Source files can be obtained from [`Schema::sources`] or [`ExecutableDocument::sources`].
    pub fn builder(
        sources: &'s SourceMap,
        main_location: Option<SourceSpan>,
        color: Color,
    ) -> Self {
        let span = main_location
            .and_then(to_span)
            .unwrap_or((FileId::NONE, 0..0));
        let report = ariadne::Report::build(ReportKind::Error, span);
        let enable_color = match color {
            Color::Never => false,
            // Rely on ariadne's `auto-color` feature, which uses `concolor` to enable colors
            // only if stderr is a terminal.
            Color::StderrIsTerminal => true,
        };
        let config = ariadne::Config::new()
            .with_index_type(ariadne::IndexType::Byte)
            .with_color(enable_color);
        Self {
            sources,
            colors: ColorGenerator::new(),
            report: report.with_config(config),
        }
    }

    /// Set the main message for the report.
    pub fn with_message(&mut self, message: impl ToString) {
        self.report.set_message(message);
    }

    /// Set the help message for the report, usually a suggestion on how to fix the error.
    pub fn with_help(&mut self, help: impl ToString) {
        self.report.set_help(help);
    }

    /// Set a note for the report, providing additional information that isn't related to a
    /// source location (when a label should be used).
    pub fn with_note(&mut self, note: impl ToString) {
        self.report.set_note(note);
    }

    /// Add a label at a given location. If the location is `None`, the message is discarded.
    pub fn with_label_opt(&mut self, location: Option<SourceSpan>, message: impl ToString) {
        if let Some(span) = location.and_then(to_span) {
            self.report.add_label(
                ariadne::Label::new(span)
                    .with_message(message)
                    .with_color(self.colors.next()),
            );
        }
    }

    /// Write the report to a [`Write`].
    ///
    /// [`Write`]: std::io::Write
    pub fn write(self, w: impl std::io::Write) -> std::io::Result<()> {
        let report = self.report.finish();
        report.write(Cache(self.sources), w)
    }

    /// Write the report to a [`fmt::Formatter`].
    pub fn fmt(self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(WriteToFormatter { f }).map_err(|_| fmt::Error)
    }

    /// Write the report to a new [`String`]
    pub fn into_string(self) -> String {
        struct OneTimeDisplay<'s>(Cell<Option<CliReport<'s>>>);

        impl fmt::Display for OneTimeDisplay<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.take().unwrap().fmt(f)
            }
        }

        OneTimeDisplay(Cell::new(Some(self))).to_string()
    }
}

struct Cache<'a>(&'a SourceMap);

impl ariadne::Cache<FileId> for Cache<'_> {
    type Storage = String;

    fn fetch(&mut self, file_id: &FileId) -> Result<&ariadne::Source, impl fmt::Debug> {
        struct NotFound(FileId);
        impl fmt::Debug for NotFound {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "source file not found: {:?}", self.0)
            }
        }
        if let Some(source_file) = self.0.get(file_id) {
            Ok(source_file.ariadne())
        } else if *file_id == FileId::NONE {
            static EMPTY: OnceLock<ariadne::Source> = OnceLock::new();
            Ok(EMPTY.get_or_init(|| ariadne::Source::from(String::new())))
        } else {
            Err(NotFound(*file_id))
        }
    }

    fn display<'a>(&self, file_id: &'a FileId) -> Option<impl fmt::Display + 'a> {
        enum Path {
            SourceFile(Arc<SourceFile>),
            NoSourceFile,
        }
        impl fmt::Display for Path {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Path::SourceFile(source_file) => source_file.path().display().fmt(f),
                    Path::NoSourceFile => f.write_str("(no source file)"),
                }
            }
        }

        if *file_id != FileId::NONE {
            let source_file = self.0.get(file_id)?;
            Some(Path::SourceFile(source_file.clone()))
        } else {
            Some(Path::NoSourceFile)
        }
    }
}

impl<T: ToCliReport> std::error::Error for Diagnostic<'_, T> {}

impl<T: ToCliReport> Diagnostic<'_, T> {
    /// Get the line and column numbers where this diagnostic spans.
    pub fn line_column_range(&self) -> Option<Range<LineColumn>> {
        self.error.location()?.line_column_range(self.sources)
    }

    /// Get a [`serde`]-serializable version of the current diagnostic. The shape is compatible
    /// with the JSON error shape described in [the GraphQL spec].
    ///
    /// [the GraphQL spec]: https://spec.graphql.org/draft/#sec-Errors
    pub fn to_json(&self) -> GraphQLError
    where
        T: ToString,
    {
        GraphQLError::new(self.error.to_string(), self.error.location(), self.sources)
    }

    /// Produce the diagnostic report, optionally with colors for the CLI.
    pub fn to_report(&self, color: Color) -> CliReport<'_> {
        self.error.to_report(self.sources, color)
    }
}

impl<T: ToCliReport> fmt::Debug for Diagnostic<'_, T> {
    /// Pretty-format the diagnostic, with colors for the CLI.
    ///
    /// The debug formatting expects to be written to stderr and ANSI colors are used if stderr is
    /// a terminal.
    ///
    /// To output *without* colors, format with `Display`: `format!("{diagnostic}")`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_report(Color::StderrIsTerminal).fmt(f)
    }
}

impl<T: ToCliReport> fmt::Display for Diagnostic<'_, T> {
    /// Pretty-format the diagnostic without colors.
    ///
    /// To output *with* colors, format with `Debug`: `eprintln!("{diagnostic:?}")`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_report(Color::Never).fmt(f)
    }
}
