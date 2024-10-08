use crate::parser::grammar::directive;
use crate::parser::grammar::enum_;
use crate::parser::grammar::extensions;
use crate::parser::grammar::fragment;
use crate::parser::grammar::input;
use crate::parser::grammar::interface;
use crate::parser::grammar::object;
use crate::parser::grammar::operation;
use crate::parser::grammar::scalar;
use crate::parser::grammar::schema;
use crate::parser::grammar::union_;
use crate::Parser;
use crate::SyntaxKind;
use crate::TokenKind;
use std::ops::ControlFlow;

/// See: https://spec.graphql.org/October2021/#Document
///
/// *Document*
///     Definition*
pub(crate) fn document(p: &mut Parser) {
    let doc = p.start_node(SyntaxKind::DOCUMENT);

    if let None | Some(TokenKind::Eof) = p.peek() {
        p.err("Unexpected <EOF>.");
    }

    p.peek_while(|p, kind| {
        assert_eq!(
            p.recursion_limit.current, 0,
            "unbalanced limit increment / decrement"
        );

        match kind {
            TokenKind::StringValue => {
                if let Some(def) = p.peek_data_n(2) {
                    select_definition(def, p);
                } else {
                    p.err_and_pop("expected a definition after this StringValue");
                }
            }
            TokenKind::Name => {
                let def = p.peek_data().unwrap();
                select_definition(def, p);
            }
            TokenKind::LCurly => {
                let def = p.peek_data().unwrap();
                select_definition(def, p);
            }
            TokenKind::Eof => return ControlFlow::Break(()),
            _ => p.err_and_pop("expected a StringValue, Name or OperationDefinition"),
        }

        ControlFlow::Continue(())
    });

    p.push_ignored();

    doc.finish_node();
}

fn select_definition(def: &str, p: &mut Parser) {
    match def {
        "directive" => directive::directive_definition(p),
        "enum" => enum_::enum_type_definition(p),
        "extend" => extensions::extensions(p),
        "fragment" => fragment::fragment_definition(p),
        "input" => input::input_object_type_definition(p),
        "interface" => interface::interface_type_definition(p),
        "type" => object::object_type_definition(p),
        "query" | "mutation" | "subscription" | "{" => operation::operation_definition(p),
        "scalar" => scalar::scalar_type_definition(p),
        "schema" => schema::schema_definition(p),
        "union" => union_::union_type_definition(p),
        _ => p.err_and_pop("expected definition"),
    }
}

#[cfg(test)]
mod test {
    use crate::cst;
    use crate::cst::CstNode;
    use crate::Parser;

    #[test]
    fn it_returns_the_original_source_string() {
        let schema = r#"
type Query {
  hero: Character
}

type Character {
  name: String
  friends: [Character]
  homeWorld: Planet
  species: Species
}

type Planet {
  name: String
  climate: String
}

type Species {
  name: String
  lifespan: Int
  origin: Planet
}
"#;

        let parser = Parser::new(schema);
        let cst = parser.parse();

        assert_eq!(cst.errors().len(), 0);
        assert_eq!(cst.document().source_string(), schema);
    }

    #[test]
    fn it_creates_error_for_invalid_definition_and_has_nodes_for_valid_definition() {
        let schema = r#"
uasdf21230jkdw

{
    pet
    faveSnack
}
        "#;
        let parser = Parser::new(schema);

        let cst = parser.parse();

        assert_eq!(cst.errors().len(), 1);
        assert_eq!(cst.document().definitions().count(), 1);
    }

    #[test]
    fn it_creates_an_error_for_a_document_with_only_an_invalid_definition() {
        let schema = r#"dtzt7777777777t7777777777z7"#;
        let parser = Parser::new(schema);

        let cst = parser.parse();
        assert_eq!(cst.errors().len(), 1);

        let doc = cst.document();
        assert!(doc.definitions().next().is_none());
    }

    #[test]
    fn it_accesses_definition_names() {
        let schema = r#"
directive @tag(name: String!) repeatable on FIELD_DEFINITION

type ProductVariation {
  id: ID!
}
scalar UUID @specifiedBy(url: "https://tools.ietf.org/html/rfc4122")

union SearchResult = Photo | Person

extend type Query {
  allProducts: [Product]
  product(id: ID!): Product
}
        "#;
        let parser = crate::Parser::new(schema);
        let cst = parser.parse();

        assert!(cst.errors.is_empty());
        let document = cst.document();
        for definition in document.definitions() {
            match definition {
                cst::Definition::DirectiveDefinition(directive) => {
                    assert_eq!(
                        directive
                            .name()
                            .expect("Cannot get directive name.")
                            .text()
                            .as_ref(),
                        "tag"
                    )
                }
                cst::Definition::ObjectTypeDefinition(object_type) => {
                    assert_eq!(
                        object_type
                            .name()
                            .expect("Cannot get object type definition name.")
                            .text()
                            .as_ref(),
                        "ProductVariation"
                    )
                }
                cst::Definition::UnionTypeDefinition(union_type) => {
                    assert_eq!(
                        union_type
                            .name()
                            .expect("Cannot get union type definition name.")
                            .text()
                            .as_ref(),
                        "SearchResult"
                    )
                }
                cst::Definition::ScalarTypeDefinition(scalar_type) => {
                    assert_eq!(
                        scalar_type
                            .name()
                            .expect("Cannot get scalar type definition name.")
                            .text()
                            .as_ref(),
                        "UUID"
                    )
                }
                cst::Definition::ObjectTypeExtension(object_type) => {
                    assert_eq!(
                        object_type
                            .name()
                            .expect("Cannot get object type extension name.")
                            .text()
                            .as_ref(),
                        "Query"
                    )
                }
                _ => unimplemented!(),
            }
        }
    }

    #[test]
    fn core_schema() {
        let schema = r#"
schema @core(feature: "https://specs.apollo.dev/join/v0.1") {
  query: Query
  mutation: Mutation
}

enum join__Graph {
  ACCOUNTS @join__graph(name: "accounts")
}
        "#;
        let parser = crate::Parser::new(schema);
        let cst = parser.parse();

        assert!(cst.errors.is_empty());

        let document = cst.document();
        for definition in document.definitions() {
            if let cst::Definition::EnumTypeDefinition(enum_type) = definition {
                let enum_name = enum_type
                    .name()
                    .expect("Could not get Enum Type Definition's Name");

                assert_eq!("join__Graph", enum_name.text().as_ref());

                if enum_name.text().as_ref() == "join__Graph" {
                    if let Some(enums) = enum_type.enum_values_definition() {
                        for enum_kind in enums.enum_value_definitions() {
                            assert_eq!(
                                "ACCOUNTS",
                                enum_kind
                                    .enum_value()
                                    .unwrap()
                                    .name()
                                    .unwrap()
                                    .text()
                                    .as_ref()
                            );
                            check_directive_arguments(enum_kind);
                        }
                    }
                }
            }
        }

        fn check_directive_arguments(enum_kind: cst::EnumValueDefinition) {
            if let Some(directives) = enum_kind.directives() {
                for directive in directives.directives() {
                    if directive
                        .name()
                        .and_then(|n| n.ident_token())
                        .as_ref()
                        .map(|id| id.text())
                        == Some("join__graph")
                    {
                        for argument in directive.arguments().unwrap().arguments() {
                            if let cst::Value::StringValue(val) =
                                argument.value().expect("Cannot get argument value.")
                            {
                                let val: String = val.into();
                                assert_eq!("accounts".to_string(), val);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn trailing_description_at_limit() {
        let parser = crate::Parser::new(
            r#"
            "All our queries"
            type Query {
                a: Int
            }

            "Imagine another type below!"
        "#,
        )
        .token_limit(18);
        // Must not panic
        let _cst = parser.parse();
    }
}
