use crate::parser::grammar::argument;
use crate::parser::grammar::description;
use crate::parser::grammar::input;
use crate::parser::grammar::name;
use crate::parser::grammar::value::Constness;
use crate::Parser;
use crate::SyntaxKind;
use crate::TokenKind;
use crate::S;
use crate::T;
use std::ops::ControlFlow;

/// See: https://spec.graphql.org/October2021/#DirectiveDefinition
///
/// *DirectiveDefinition*:
///     Description? **directive @** Name ArgumentsDefinition? **repeatable**? **on** DirectiveLocations
pub(crate) fn directive_definition(p: &mut Parser) {
    let _g = p.start_node(SyntaxKind::DIRECTIVE_DEFINITION);

    if let Some(TokenKind::StringValue) = p.peek() {
        description::description(p);
    }

    if let Some("directive") = p.peek_data() {
        p.bump(SyntaxKind::directive_KW);
    }

    match p.peek() {
        Some(T![@]) => p.bump(S![@]),
        _ => p.err("expected @ symbol"),
    }
    name::name(p);

    if let Some(T!['(']) = p.peek() {
        let _g = p.start_node(SyntaxKind::ARGUMENTS_DEFINITION);
        p.bump(S!['(']);
        if let Some(TokenKind::Name | TokenKind::StringValue) = p.peek() {
            input::input_value_definition(p);
        } else {
            p.err("expected an Argument Definition");
        }
        p.peek_while(|p, kind| match kind {
            TokenKind::Name | TokenKind::StringValue => {
                input::input_value_definition(p);
                ControlFlow::Continue(())
            }
            _ => ControlFlow::Break(()),
        });
        p.expect(T![')'], S![')']);
    }

    if let Some(node) = p.peek_data() {
        if node == "repeatable" {
            p.bump(SyntaxKind::repeatable_KW);
        }
    }

    if let Some(node) = p.peek_data() {
        match node {
            "on" => p.bump(SyntaxKind::on_KW),
            _ => p.err("expected Directive Locations"),
        }
    }

    if let Some(TokenKind::Name | T![|]) = p.peek() {
        let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATIONS);
        directive_locations(p);
    } else {
        p.err("expected valid Directive Location");
    }
}

/// https://spec.graphql.org/October2021/#DirectiveLocation
///
/// *DirectiveLocation*:
///     *ExecutableDirectiveLocation*
///     *TypeSystemDirectiveLocation*
///
/// (This function does not distinguish between the two groups of
/// locations.)
fn directive_location(p: &mut Parser) {
    let Some(token) = p.peek_token() else {
        return;
    };

    if token.kind == TokenKind::Name {
        match token.data {
            "QUERY" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::QUERY_KW);
            }
            "MUTATION" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::MUTATION_KW);
            }
            "SUBSCRIPTION" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::SUBSCRIPTION_KW);
            }
            "FIELD" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::FIELD_KW);
            }
            "FRAGMENT_DEFINITION" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::FRAGMENT_DEFINITION_KW);
            }
            "FRAGMENT_SPREAD" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::FRAGMENT_SPREAD_KW);
            }
            "INLINE_FRAGMENT" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::INLINE_FRAGMENT_KW);
            }
            "VARIABLE_DEFINITION" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::VARIABLE_DEFINITION_KW);
            }
            "SCHEMA" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::SCHEMA_KW);
            }
            "SCALAR" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::SCALAR_KW);
            }
            "OBJECT" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::OBJECT_KW);
            }
            "FIELD_DEFINITION" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::FIELD_DEFINITION_KW);
            }
            "ARGUMENT_DEFINITION" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::ARGUMENT_DEFINITION_KW);
            }
            "INTERFACE" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::INTERFACE_KW);
            }
            "UNION" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::UNION_KW);
            }
            "ENUM" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::ENUM_KW);
            }
            "ENUM_VALUE" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::ENUM_VALUE_KW);
            }
            "INPUT_OBJECT" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::INPUT_OBJECT_KW);
            }
            "INPUT_FIELD_DEFINITION" => {
                let _g = p.start_node(SyntaxKind::DIRECTIVE_LOCATION);
                p.bump(SyntaxKind::INPUT_FIELD_DEFINITION_KW);
            }
            _ => {
                p.err("expected valid Directive Location");
            }
        }
    } else {
        p.err("expected Directive Location");
    }
}

/// See: https://spec.graphql.org/October2021/#DirectiveLocations
///
/// *DirectiveLocations*:
///     DirectiveLocations **|** DirectiveLocation
///     **|**? DirectiveLocation
pub(crate) fn directive_locations(p: &mut Parser) {
    p.parse_separated_list(T![|], S![|], directive_location);
}

/// See: https://spec.graphql.org/October2021/#Directive
///
/// *Directive[Const]*:
///     **@** Name Arguments[?Const]?
pub(crate) fn directive(p: &mut Parser, constness: Constness) {
    let _g = p.start_node(SyntaxKind::DIRECTIVE);

    p.expect(T![@], S![@]);
    name::name(p);

    if let Some(T!['(']) = p.peek() {
        argument::arguments(p, constness);
    }
}

/// See: https://spec.graphql.org/October2021/#Directives
///
/// *Directives[Const]*:
///     Directive[?Const]*
pub(crate) fn directives(p: &mut Parser, constness: Constness) {
    let _g = p.start_node(SyntaxKind::DIRECTIVES);
    p.peek_while_kind(T![@], |p| {
        directive(p, constness);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cst;

    #[test]
    fn it_can_access_repeatable_kw_on_directive_definition() {
        let schema = r#"
directive @example(isTreat: Boolean, treatKind: String) repeatable on FIELD | MUTATION
        "#;
        let parser = Parser::new(schema);
        let cst = parser.parse();

        assert!(cst.errors.is_empty());

        let document = cst.document();
        for definition in document.definitions() {
            if let cst::Definition::DirectiveDefinition(dir_def) = definition {
                assert_eq!(
                    dir_def.repeatable_token().unwrap().kind(),
                    SyntaxKind::repeatable_KW
                );
                return;
            }
        }
        panic!("Expected CST to have a Directive Definition");
    }

    #[test]
    fn it_can_access_directive_location_on_directive_definition() {
        let schema = r#"
directive @example on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
        "#;
        let parser = Parser::new(schema);
        let cst = parser.parse();
        assert!(cst.errors.is_empty());

        let document = cst.document();
        for definition in document.definitions() {
            if let cst::Definition::DirectiveDefinition(dir_def) = definition {
                let dir_locations: Vec<String> = dir_def
                    .directive_locations()
                    .unwrap()
                    .directive_locations()
                    .map(|loc| loc.text().unwrap().to_string())
                    .collect();
                assert_eq!(
                    dir_locations,
                    ["FIELD", "FRAGMENT_SPREAD", "INLINE_FRAGMENT"]
                );
                return;
            }
        }
        panic!("Expected CST to have a Directive Definition");
    }

    #[test]
    fn it_can_access_multiline_directive_location_on_directive_definition() {
        let schema = r#"
directive @example on
| FIELD
| FRAGMENT_SPREAD
| INLINE_FRAGMENT
        "#;
        let parser = Parser::new(schema);
        let cst = parser.parse();
        assert!(cst.errors.is_empty());

        let document = cst.document();
        for definition in document.definitions() {
            if let cst::Definition::DirectiveDefinition(dir_def) = definition {
                let dir_locations: Vec<String> = dir_def
                    .directive_locations()
                    .unwrap()
                    .directive_locations()
                    .map(|loc| loc.text().unwrap().to_string())
                    .collect();
                assert_eq!(
                    dir_locations,
                    ["FIELD", "FRAGMENT_SPREAD", "INLINE_FRAGMENT"]
                );
                return;
            }
        }
        panic!("Expected CST to have a Directive Definition");
    }

    #[test]
    fn it_can_access_multiline_directive_location_on_directive_definition_with_error() {
        let schema = r#"
directive @example on
| FIELD
| FRAGMENT_SPREAD
| INLINE_FRAGMENT |
        "#;
        let parser = Parser::new(schema);
        let cst = parser.parse();
        assert!(!cst.errors.is_empty());

        let schema = r#"
directive @example on
|| FIELD
| FRAGMENT_SPREAD
| INLINE_FRAGMENT
        "#;
        let parser = Parser::new(schema);
        let cst = parser.parse();
        assert!(!cst.errors.is_empty());

        let schema = r#"
directive @example on
| FIELD
|| FRAGMENT_SPREAD
| INLINE_FRAGMENT
        "#;
        let parser = Parser::new(schema);
        let cst = parser.parse();
        assert!(!cst.errors.is_empty());
    }
}
