use super::LimitTracker;
use crate::cst;
use crate::cst::CstNode;
use crate::Error;
use crate::SyntaxElement;
use crate::SyntaxKind;
use crate::SyntaxNode;
use rowan::GreenNode;
use rowan::GreenNodeBuilder;
use std::fmt;
use std::marker::PhantomData;
use std::slice::Iter;

/// A CST generated by the parser. Consists of a syntax tree and a `Vec<Error>`
/// if any.
///
/// ## Example
///
/// Given a syntactically incorrect token `uasdf21230jkdw` which cannot be part
/// of any of GraphQL definitions and a syntactically correct SelectionSet, we
/// are able to see both the CST for the SelectionSet and the error with an
/// incorrect token.
/// ```rust
/// use apollo_parser::Parser;
///
/// let schema = r#"
/// uasdf21230jkdw
///
/// {
///   pet
///   faveSnack
/// }
/// "#;
/// let parser = Parser::new(schema);
///
/// let cst = parser.parse();
/// // The Vec<Error> that's part of the SyntaxTree struct.
/// assert_eq!(cst.errors().len(), 1);
///
/// // The CST with Document as its root node.
/// let doc = cst.document();
/// let nodes: Vec<_> = doc.definitions().into_iter().collect();
/// assert_eq!(nodes.len(), 1);
/// ```

// NOTE(@lrlna): This enum helps us setup a type state for document and field
// set parsing.  Without the wrapper we'd have to add type annotations to
// SyntaxTreeBuilder, which then annotates the vast majority of the parser's
// function with the same type parameter. This is tedious and makes for a more
// complex design than necessary.
pub(crate) enum SyntaxTreeWrapper {
    Document(SyntaxTree<cst::Document>),
    FieldSet(SyntaxTree<cst::SelectionSet>),
    Type(SyntaxTree<cst::Type>),
}

#[derive(PartialEq, Eq, Clone)]
pub struct SyntaxTree<T: CstNode = cst::Document> {
    pub(crate) green: GreenNode,
    pub(crate) errors: Vec<crate::Error>,
    pub(crate) recursion_limit: LimitTracker,
    pub(crate) token_limit: LimitTracker,
    _phantom: PhantomData<fn() -> T>,
}

const _: () = {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    let _ = assert_send::<SyntaxTree>;
    let _ = assert_sync::<SyntaxTree>;
};

impl<T: CstNode> SyntaxTree<T> {
    /// Get a reference to the syntax tree's errors.
    pub fn errors(&self) -> Iter<'_, crate::Error> {
        self.errors.iter()
    }

    /// Get the syntax tree's recursion limit.
    pub fn recursion_limit(&self) -> LimitTracker {
        self.recursion_limit
    }

    /// Get the syntax tree's token limit.
    pub fn token_limit(&self) -> LimitTracker {
        self.token_limit
    }

    pub fn green(&self) -> GreenNode {
        self.green.clone()
    }

    pub(crate) fn syntax_node(&self) -> SyntaxNode {
        rowan::SyntaxNode::new_root(self.green.clone())
    }
}

impl SyntaxTree<cst::Document> {
    /// Return the root typed `Document` node.
    pub fn document(&self) -> cst::Document {
        cst::Document {
            syntax: self.syntax_node(),
        }
    }
}

impl SyntaxTree<cst::SelectionSet> {
    /// Return the root typed `SelectionSet` node. This is used for parsing
    /// selection sets defined by @requires directive.
    pub fn field_set(&self) -> cst::SelectionSet {
        cst::SelectionSet {
            syntax: self.syntax_node(),
        }
    }
}

impl SyntaxTree<cst::Type> {
    /// Return the root typed `SelectionSet` node. This is used for parsing
    /// selection sets defined by @requires directive.
    pub fn ty(&self) -> cst::Type {
        match self.syntax_node().kind() {
            SyntaxKind::NAMED_TYPE => cst::Type::NamedType(cst::NamedType {
                syntax: self.syntax_node(),
            }),
            SyntaxKind::LIST_TYPE => cst::Type::ListType(cst::ListType {
                syntax: self.syntax_node(),
            }),
            SyntaxKind::NON_NULL_TYPE => cst::Type::NonNullType(cst::NonNullType {
                syntax: self.syntax_node(),
            }),
            _ => unreachable!("this should only return Type node"),
        }
    }
}

impl<T: CstNode> fmt::Debug for SyntaxTree<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print(f: &mut fmt::Formatter<'_>, indent: usize, element: SyntaxElement) -> fmt::Result {
            let kind: SyntaxKind = element.kind();
            write!(f, "{:indent$}", "", indent = indent)?;
            match element {
                rowan::NodeOrToken::Node(node) => {
                    writeln!(f, "- {:?}@{:?}", kind, node.text_range())?;
                    for child in node.children_with_tokens() {
                        print(f, indent + 4, child)?;
                    }
                    Ok(())
                }

                rowan::NodeOrToken::Token(token) => {
                    writeln!(
                        f,
                        "- {:?}@{:?} {:?}",
                        kind,
                        token.text_range(),
                        token.text()
                    )
                }
            }
        }

        fn print_err(f: &mut fmt::Formatter<'_>, errors: Vec<Error>) -> fmt::Result {
            for err in errors {
                writeln!(f, "- {err:?}")?;
            }

            write!(f, "")
        }

        fn print_recursion_limit(
            f: &mut fmt::Formatter<'_>,
            recursion_limit: LimitTracker,
        ) -> fmt::Result {
            write!(f, "{recursion_limit:?}")
        }

        print(f, 0, self.syntax_node().into())?;
        print_err(f, self.errors.clone())?;
        print_recursion_limit(f, self.recursion_limit)
    }
}

#[derive(Debug)]
pub(crate) struct SyntaxTreeBuilder {
    builder: GreenNodeBuilder<'static>,
}

impl SyntaxTreeBuilder {
    /// Create a new instance of `SyntaxBuilder`.
    pub(crate) fn new() -> Self {
        Self {
            builder: GreenNodeBuilder::new(),
        }
    }

    pub(crate) fn checkpoint(&self) -> rowan::Checkpoint {
        self.builder.checkpoint()
    }

    /// Start new node and make it current.
    pub(crate) fn start_node(&mut self, kind: SyntaxKind) {
        self.builder.start_node(rowan::SyntaxKind(kind as u16));
    }

    /// Finish current branch and restore previous branch as current.
    pub(crate) fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    pub(crate) fn wrap_node(&mut self, checkpoint: rowan::Checkpoint, kind: SyntaxKind) {
        self.builder
            .start_node_at(checkpoint, rowan::SyntaxKind(kind as u16));
    }

    /// Adds new token to the current branch.
    pub(crate) fn token(&mut self, kind: SyntaxKind, text: &str) {
        self.builder.token(rowan::SyntaxKind(kind as u16), text);
    }

    pub(crate) fn finish_document(
        self,
        errors: Vec<Error>,
        recursion_limit: LimitTracker,
        token_limit: LimitTracker,
    ) -> SyntaxTreeWrapper {
        SyntaxTreeWrapper::Document(SyntaxTree {
            green: self.builder.finish(),
            // TODO: keep the errors in the builder rather than pass it in here?
            errors,
            // TODO: keep the recursion and token limits in the builder rather than pass it in here?
            recursion_limit,
            token_limit,
            _phantom: PhantomData,
        })
    }

    pub(crate) fn finish_selection_set(
        self,
        errors: Vec<Error>,
        recursion_limit: LimitTracker,
        token_limit: LimitTracker,
    ) -> SyntaxTreeWrapper {
        SyntaxTreeWrapper::FieldSet(SyntaxTree {
            green: self.builder.finish(),
            // TODO: keep the errors in the builder rather than pass it in here?
            errors,
            // TODO: keep the recursion and token limits in the builder rather than pass it in here?
            recursion_limit,
            token_limit,
            _phantom: PhantomData,
        })
    }

    pub(crate) fn finish_type(
        self,
        errors: Vec<Error>,
        recursion_limit: LimitTracker,
        token_limit: LimitTracker,
    ) -> SyntaxTreeWrapper {
        SyntaxTreeWrapper::Type(SyntaxTree {
            green: self.builder.finish(),
            // TODO: keep the errors in the builder rather than pass it in here?
            errors,
            // TODO: keep the recursion and token limits in the builder rather than pass it in here?
            recursion_limit,
            token_limit,
            _phantom: PhantomData,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::cst::Definition;
    use crate::Parser;

    #[test]
    fn directive_name() {
        let input = "directive @example(isTreat: Boolean, treatKind: String) on FIELD | MUTATION";
        let parser = Parser::new(input);
        let cst = parser.parse();
        let doc = cst.document();

        for def in doc.definitions() {
            if let Definition::DirectiveDefinition(directive) = def {
                assert_eq!(directive.name().unwrap().text(), "example");
            }
        }
    }

    #[test]
    fn object_type_definition() {
        let input = "
        type ProductDimension {
          size: String
          weight: Float @tag(name: \"hi from inventory value type field\")
        }
        ";
        let parser = Parser::new(input);
        let cst = parser.parse();
        assert_eq!(0, cst.errors().len());

        let doc = cst.document();

        for def in doc.definitions() {
            if let Definition::ObjectTypeDefinition(object_type) = def {
                assert_eq!(object_type.name().unwrap().text(), "ProductDimension");
                for field_def in object_type.fields_definition().unwrap().field_definitions() {
                    println!("{}", field_def.name().unwrap().text()); // size weight
                }
            }
        }
    }
}
