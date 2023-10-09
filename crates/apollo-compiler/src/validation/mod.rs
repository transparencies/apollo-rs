mod validation_db;

mod argument;
mod directive;
mod enum_;
mod field;
mod fragment;
mod input_object;
mod interface;
mod object;
pub(crate) mod operation;
mod scalar;
mod schema;
pub(crate) mod selection;
mod union_;
mod value;
mod variable;

use crate::executable::BuildError as ExecutableBuildError;
use crate::schema::BuildError as SchemaBuildError;
use crate::FileId;
use crate::NodeLocation;
use crate::NodeStr;
use crate::SourceFile;
use crate::SourceMap;
use indexmap::IndexSet;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::OnceLock;
pub(crate) use validation_db::{ValidationDatabase, ValidationStorage};

pub struct Diagnostics(Box<DiagnosticsBoxed>);

/// Box indirection to avoid large `Err` values:
/// <https://rust-lang.github.io/rust-clippy/master/index.html#result_large_err>
struct DiagnosticsBoxed {
    sources: Sources,
    errors: Vec<Error>,
}

struct Sources {
    schema_sources: Option<SourceMap>,
    self_sources: SourceMap,
}

struct Error {
    location: Option<NodeLocation>,
    details: Details,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum Details {
    #[error("{message}")]
    ParserLimit { message: String },
    #[error("syntax error: {message}")]
    SyntaxError { message: String },
    #[error("{0}")]
    SchemaBuildError(SchemaBuildError),
    #[error("{0}")]
    ExecutableBuildError(ExecutableBuildError),
    #[error("syntax error: {0}")]
    CompilerDiagnostic(crate::ApolloDiagnostic),
}

impl Error {
    fn report(&self, color: bool) -> ariadne::Report<'static, NodeLocation> {
        let config = ariadne::Config::default().with_color(color);
        if let Details::CompilerDiagnostic(diagnostic) = &self.details {
            return diagnostic.to_report(config);
        }

        let (id, offset) = if let Some(location) = self.location {
            (location.file_id(), location.offset())
        } else {
            (FileId::NONE, 0)
        };

        let mut report = ariadne::Report::build::<FileId>(ariadne::ReportKind::Error, id, offset)
            .with_config(config);
        let mut colors = ariadne::ColorGenerator::new();
        macro_rules! opt_label {
            ($location: expr, $message: literal $(, $args: expr )* $(,)?) => {
                if let Some(location) = $location {
                    report.add_label(
                        ariadne::Label::new(*location)
                            .with_message(format_args!($message $(, $args)*))
                            .with_color(colors.next()),
                    )
                }
            };
            ($message: literal $(, $args: expr )* $(,)?) => {
                opt_label!(&self.location, $message $(, $args)*)
            };
        }
        // Main message from `derive(thiserror::Error)` based on `#[error("…")]` attributes:
        report.set_message(&self.details);
        // Every case should also have a label at the main location
        // (preferably saying something not completely redundant with the main message)
        // and may have additional labels.
        // Labels are always optional because locations are always optional,
        // so essential information should be in the main message.
        match &self.details {
            Details::CompilerDiagnostic(_) => unreachable!(),
            Details::ParserLimit { message, .. } => opt_label!("{message}"),
            Details::SyntaxError { message, .. } => opt_label!("{message}"),
            Details::SchemaBuildError(err) => match err {
                SchemaBuildError::ExecutableDefinition { .. } => {
                    opt_label!("remove this definition, or use `parse_mixed()`")
                }
                SchemaBuildError::SchemaDefinitionCollision {
                    previous_location, ..
                } => {
                    opt_label!(previous_location, "previous `schema` definition here");
                    opt_label!("`schema` redefined here");
                    report.set_help(
                        "merge this definition with the previous one, or use `extend schema`",
                    );
                }
                SchemaBuildError::DirectiveDefinitionCollision {
                    previous_location,
                    name,
                    ..
                } => {
                    opt_label!(previous_location, "previous definition of `@{name}` here");
                    opt_label!("`@{name}` redefined here");
                    report.set_help("remove or rename one of the definitions");
                }
                SchemaBuildError::TypeDefinitionCollision {
                    previous_location,
                    name,
                    ..
                } => {
                    opt_label!(previous_location, "previous definition of `{name}` here");
                    opt_label!("`{name}` redefined here");
                    report.set_help("remove or rename one of the definitions, or use `extend`");
                }
                SchemaBuildError::BuiltInScalarTypeRedefinition { .. } => {
                    opt_label!("remove this scalar definition");
                }
                SchemaBuildError::OrphanSchemaExtension { .. } => opt_label!("extension here"),
                SchemaBuildError::OrphanTypeExtension { .. } => opt_label!("extension here"),
                SchemaBuildError::TypeExtensionKindMismatch { def_location, .. } => {
                    opt_label!(def_location, "type definition");
                    opt_label!("extension here")
                }
                SchemaBuildError::DuplicateRootOperation {
                    previous_location,
                    operation_type,
                    ..
                } => {
                    opt_label!(
                        previous_location,
                        "previous definition of `{operation_type}` here"
                    );
                    opt_label!("`{operation_type}` redefined here");
                }
                SchemaBuildError::DuplicateImplementsInterfaceInObject {
                    name_at_previous_location,
                    ..
                }
                | SchemaBuildError::DuplicateImplementsInterfaceInInterface {
                    name_at_previous_location,
                    ..
                } => {
                    let previous_location = &name_at_previous_location.location();
                    let name = name_at_previous_location;
                    opt_label!(
                        previous_location,
                        "previous implementation of `{name}` here"
                    );
                    opt_label!("`{name}` implemented again here");
                }
                SchemaBuildError::ObjectFieldNameCollision {
                    name_at_previous_location,
                    ..
                }
                | SchemaBuildError::InterfaceFieldNameCollision {
                    name_at_previous_location,
                    ..
                }
                | SchemaBuildError::EnumValueNameCollision {
                    name_at_previous_location,
                    ..
                }
                | SchemaBuildError::UnionMemberNameCollision {
                    name_at_previous_location,
                    ..
                }
                | SchemaBuildError::InputFieldNameCollision {
                    name_at_previous_location,
                    ..
                } => {
                    let previous_location = &name_at_previous_location.location();
                    let name = name_at_previous_location;
                    opt_label!(previous_location, "previous definition of `{name}` here");
                    opt_label!("`{name}` redefined here");
                }
            },
            Details::ExecutableBuildError(err) => match err {
                ExecutableBuildError::TypeSystemDefinition { .. } => {
                    opt_label!("remove this definition, or use `parse_mixed()`")
                }
                ExecutableBuildError::AmbiguousAnonymousOperation { .. } => {
                    opt_label!("provide a name for this definition");
                    report.set_help(
                        "GraphQL requires operations to be named if the document has more than one",
                    );
                }
                ExecutableBuildError::OperationNameCollision {
                    name_at_previous_location,
                    ..
                }
                | ExecutableBuildError::FragmentNameCollision {
                    name_at_previous_location,
                    ..
                } => {
                    let previous_location = &name_at_previous_location.location();
                    let name = name_at_previous_location;
                    opt_label!(previous_location, "previous definition of `{name}` here");
                    opt_label!("`{name}` redefined here");
                }
                ExecutableBuildError::UndefinedRootOperation { operation_type, .. } => {
                    opt_label!(
                        "`{operation_type}` is not defined in the schema \
                         and is therefore not supported"
                    );
                    report.set_help(format_args!(
                        "consider defining a `{operation_type}` root operation type \
                         in your schema"
                    ))
                }
                ExecutableBuildError::UndefinedTypeInNamedFragmentTypeCondition { .. } => {
                    opt_label!("type condition here")
                }
                ExecutableBuildError::UndefinedTypeInInlineFragmentTypeCondition {
                    path, ..
                } => {
                    opt_label!("type condition here");
                    report.set_note(format_args!("path to the inline fragment: `{path} → ...`"))
                }
                ExecutableBuildError::SubselectionOnScalarType { path, .. }
                | ExecutableBuildError::SubselectionOnEnumType { path, .. } => {
                    opt_label!("remove subselections here");
                    report.set_note(format_args!("path to the field: `{path}`"))
                }
                ExecutableBuildError::UndefinedField {
                    field_name,
                    type_name,
                    path,
                    ..
                } => {
                    opt_label!("field `{field_name}` selected here");
                    opt_label!(&type_name.location(), "type `{type_name}` defined here");
                    report.set_note(format_args!("path to the field: `{path}`"))
                }
            },
        }
        report.finish()
    }
}

impl Diagnostics {
    /// Returns a human-readable string formatting, without color codes regardless of stderr.
    ///
    /// `Display` and `.to_string()` are meant for printing to stderr,
    /// and will include ANSI color codes if stderr is detected to be a terminal.
    pub fn to_string_no_color(&self) -> String {
        format!("{self:#}")
    }

    pub(crate) fn new(schema_sources: Option<SourceMap>, self_sources: SourceMap) -> Self {
        Self(Box::new(DiagnosticsBoxed {
            sources: Sources {
                schema_sources,
                self_sources,
            },
            errors: Vec::new(),
        }))
    }

    pub(crate) fn push(&mut self, location: Option<NodeLocation>, details: Details) {
        self.0.errors.push(Error { location, details })
    }

    pub(crate) fn into_result(mut self) -> Result<(), Self> {
        if self.0.errors.is_empty() {
            Ok(())
        } else {
            self.sort();
            Err(self)
        }
    }

    pub(crate) fn sort(&mut self) {
        self.0
            .errors
            .sort_by_key(|err| err.location.map(|loc| (loc.file_id(), loc.offset())))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.errors.is_empty()
    }
}

/// Use alternate formatting to disable colors: `format!("{validation_errors:#}")`
impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Adaptor<'a, 'b>(&'a mut fmt::Formatter<'b>);

        impl io::Write for Adaptor<'_, '_> {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                let s = std::str::from_utf8(buf).map_err(|_| io::ErrorKind::Other)?;
                self.0.write_str(s).map_err(|_| io::ErrorKind::Other)?;
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let color = !f.alternate();
        for error in &self.0.errors {
            error
                .report(color)
                .write(&self.0.sources, Adaptor(f))
                .map_err(|_| fmt::Error)?
        }
        Ok(())
    }
}

impl Sources {
    pub(crate) fn get(&self, file_id: &FileId) -> Option<&Arc<SourceFile>> {
        self.self_sources
            .get(file_id)
            .or_else(|| self.schema_sources.as_ref()?.get(file_id))
    }
}

impl ariadne::Cache<FileId> for &'_ Sources {
    fn fetch(&mut self, file_id: &FileId) -> Result<&ariadne::Source, Box<dyn fmt::Debug + '_>> {
        struct NotFound(FileId);
        impl fmt::Debug for NotFound {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "source file not found: {:?}", self.0)
            }
        }
        if let Some(source_file) = self.get(file_id) {
            Ok(source_file.ariadne())
        } else if *file_id == FileId::NONE
            || *file_id == FileId::HACK_TMP
            || *file_id == FileId::HACK_TMP_2
        {
            static EMPTY: OnceLock<ariadne::Source> = OnceLock::new();
            Ok(EMPTY.get_or_init(|| ariadne::Source::from("")))
        } else {
            Err(Box::new(NotFound(*file_id)))
        }
    }

    fn display<'a>(&self, file_id: &'a FileId) -> Option<Box<dyn fmt::Display + 'a>> {
        if *file_id != FileId::NONE {
            struct Path(Arc<SourceFile>);
            impl fmt::Display for Path {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.0.path().display().fmt(f)
                }
            }
            let source_file = self.get(file_id)?;
            Some(Box::new(Path(source_file.clone())))
        } else {
            struct NoSourceFile;
            impl fmt::Display for NoSourceFile {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("(no source file)")
                }
            }
            Some(Box::new(NoSourceFile))
        }
    }
}

impl fmt::Debug for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl ariadne::Span for NodeLocation {
    type SourceId = FileId;

    fn source(&self) -> &FileId {
        &self.file_id
    }

    fn start(&self) -> usize {
        self.offset()
    }

    fn end(&self) -> usize {
        self.end_offset()
    }
}

/// Track used names in a recursive function.
struct RecursionStack {
    seen: IndexSet<NodeStr>,
}

impl RecursionStack {
    fn with_root(root: NodeStr) -> Self {
        let mut seen = IndexSet::new();
        seen.insert(root);
        Self { seen }
    }

    /// Return the actual API for tracking recursive uses.
    pub(crate) fn guard(&mut self) -> RecursionGuard<'_> {
        RecursionGuard(&mut self.seen)
    }
}

/// Track used names in a recursive function.
///
/// Pass the result of `guard.push(name)` to recursive calls. Use `guard.contains(name)` to check
/// if the name was used somewhere up the call stack. When a guard is dropped, its name is removed
/// from the list.
struct RecursionGuard<'a>(&'a mut IndexSet<NodeStr>);
impl RecursionGuard<'_> {
    /// Mark that we saw a name.
    fn push(&mut self, name: &NodeStr) -> RecursionGuard<'_> {
        debug_assert!(
            self.0.insert(name.clone()),
            "cannot push the same name twice to RecursionGuard, check contains() first"
        );
        RecursionGuard(self.0)
    }
    /// Check if we saw a name somewhere up the call stack.
    fn contains(&self, name: &NodeStr) -> bool {
        self.0.iter().any(|seen| seen == name)
    }
    /// Return the name where we started.
    fn first(&self) -> Option<&NodeStr> {
        self.0.first()
    }
}

impl Drop for RecursionGuard<'_> {
    fn drop(&mut self) {
        // This may already be empty if it's the original `stack.guard()` result, but that's fine
        self.0.pop();
    }
}
