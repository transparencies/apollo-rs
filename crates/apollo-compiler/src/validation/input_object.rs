use crate::ast;
use crate::collections::HashMap;
use crate::schema::validation::BuiltInScalars;
use crate::schema::InputObjectType;
use crate::validation::diagnostics::DiagnosticData;
use crate::validation::CycleError;
use crate::validation::DiagnosticList;
use crate::validation::RecursionGuard;
use crate::validation::RecursionStack;
use crate::Name;
use crate::Node;

// Implements [Circular References](https://spec.graphql.org/October2021/#sec-Input-Objects.Circular-References)
// part of the input object validation spec.
struct FindRecursiveInputValue<'a> {
    schema: &'a crate::Schema,
}

impl FindRecursiveInputValue<'_> {
    fn input_value_definition(
        &self,
        seen: &mut RecursionGuard<'_>,
        def: &Node<ast::InputValueDefinition>,
    ) -> Result<(), CycleError<ast::InputValueDefinition>> {
        match &*def.ty {
            // NonNull type followed by Named type is the one that's not allowed
            // to be cyclical, so this is only case we care about.
            //
            // Everything else may be a cyclical input value.
            ast::Type::NonNullNamed(name) => {
                if !seen.contains(name) {
                    if let Some(object_def) = self.schema.get_input_object(name) {
                        self.input_object_definition(seen.push(name)?, object_def)
                            .map_err(|err| err.trace(def))?
                    }
                } else if seen.first() == Some(name) {
                    return Err(CycleError::Recursed(vec![def.clone()]));
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn input_object_definition(
        &self,
        mut seen: RecursionGuard<'_>,
        input_object: &InputObjectType,
    ) -> Result<(), CycleError<ast::InputValueDefinition>> {
        for input_value in input_object.fields.values() {
            self.input_value_definition(&mut seen, input_value)?;
        }

        Ok(())
    }

    fn check(
        schema: &crate::Schema,
        input_object: &InputObjectType,
    ) -> Result<(), CycleError<ast::InputValueDefinition>> {
        let mut recursion_stack = RecursionStack::with_root(input_object.name.clone());
        FindRecursiveInputValue { schema }
            .input_object_definition(recursion_stack.guard(), input_object)
    }
}

pub(crate) fn validate_input_object_definition(
    diagnostics: &mut DiagnosticList,
    schema: &crate::Schema,
    built_in_scalars: &mut BuiltInScalars,
    input_object: &Node<InputObjectType>,
) {
    super::directive::validate_directives(
        diagnostics,
        Some(schema),
        input_object.directives.iter_ast(),
        ast::DirectiveLocation::InputObject,
        // input objects don't use variables
        Default::default(),
    );

    match FindRecursiveInputValue::check(schema, input_object) {
        Ok(_) => {}
        Err(CycleError::Recursed(trace)) => diagnostics.push(
            input_object.location(),
            DiagnosticData::RecursiveInputObjectDefinition {
                name: input_object.name.clone(),
                trace,
            },
        ),
        Err(CycleError::Limit(_)) => {
            diagnostics.push(
                input_object.location(),
                DiagnosticData::DeeplyNestedType {
                    name: input_object.name.clone(),
                    describe_type: "input object",
                },
            );
        }
    }

    // Fields in an Input Object Definition must be unique
    //
    // Returns Unique Definition error.
    let fields: Vec<_> = input_object
        .fields
        .values()
        .map(|c| c.node.clone())
        .collect();
    validate_input_value_definitions(
        diagnostics,
        schema,
        built_in_scalars,
        &fields,
        ast::DirectiveLocation::InputFieldDefinition,
        "an input object field",
    );

    // validate there is at least one input value on the input object type
    // https://spec.graphql.org/draft/#sel-HAHhBXDBABAB5BvgD
    if input_object.fields.is_empty() {
        diagnostics.push(
            input_object.location(),
            DiagnosticData::EmptyInputValueSet {
                type_name: input_object.name.clone(),
                type_location: input_object.location(),
                extensions_locations: input_object
                    .extensions()
                    .iter()
                    .map(|ext| ext.location())
                    .collect(),
            },
        );
    }
}

pub(crate) fn validate_argument_definitions(
    diagnostics: &mut DiagnosticList,
    schema: &crate::Schema,
    built_in_scalars: &mut BuiltInScalars,
    input_values: &[Node<ast::InputValueDefinition>],
    directive_location: ast::DirectiveLocation,
) {
    validate_input_value_definitions(
        diagnostics,
        schema,
        built_in_scalars,
        input_values,
        directive_location,
        "an argument",
    );

    let mut seen: HashMap<Name, &Node<ast::InputValueDefinition>> = HashMap::default();
    for input_value in input_values {
        let name = &input_value.name;
        if let Some(prev_value) = seen.get(name) {
            let (original_definition, redefined_definition) =
                (prev_value.location(), input_value.location());

            diagnostics.push(
                original_definition,
                DiagnosticData::UniqueInputValue {
                    name: name.clone(),
                    original_definition,
                    redefined_definition,
                },
            );
        } else {
            seen.insert(name.clone(), input_value);
        }
    }
}

pub(crate) fn validate_input_value_definitions(
    diagnostics: &mut DiagnosticList,
    schema: &crate::Schema,
    built_in_scalars: &mut BuiltInScalars,
    input_values: &[Node<ast::InputValueDefinition>],
    directive_location: ast::DirectiveLocation,
    describe: &'static str,
) {
    for input_value in input_values {
        crate::schema::validation::validate_type_system_name(
            diagnostics,
            &input_value.name,
            describe,
        );
        super::directive::validate_directives(
            diagnostics,
            Some(schema),
            input_value.directives.iter(),
            directive_location,
            Default::default(), // No variables in an input value definition
        );
        // Input values must only contain input types.
        let loc = input_value.location();
        let named_type = input_value.ty.inner_named_type();
        let is_built_in = built_in_scalars.record_type_ref(schema, named_type);
        if let Some(field_ty) = schema.types.get(named_type) {
            if !field_ty.is_input_type() {
                diagnostics.push(
                    loc,
                    DiagnosticData::InputType {
                        name: input_value.name.clone(),
                        describe_type: field_ty.describe(),
                        type_location: input_value.ty.location(),
                    },
                );
            }
            // TODO: Validate default values in apollo-compiler 2.0
            // https://github.com/apollographql/apollo-rs/issues/928
            //
            // if let Some(default) = &input_value.default_value {
            //     let var_defs = &[];
            //     value_of_correct_type(diagnostics, schema, &input_value.ty, default, var_defs);
            // }
        } else if is_built_in {
            // `validate_schema()` will insert the missing definition
        } else {
            let loc = named_type.location();
            diagnostics.push(
                loc,
                DiagnosticData::UndefinedDefinition {
                    name: named_type.clone(),
                },
            );
        }
    }
}
