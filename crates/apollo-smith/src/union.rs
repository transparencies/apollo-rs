use crate::description::Description;
use crate::directive::Directive;
use crate::directive::DirectiveLocation;
use crate::name::Name;
use crate::ty::Ty;
use crate::DocumentBuilder;
use apollo_compiler::ast;
use arbitrary::Result as ArbitraryResult;
use indexmap::IndexMap;
use indexmap::IndexSet;

/// UnionDefs are an abstract type where no common fields are declared.
///
/// *UnionDefTypeDefinition*:
///     Description? **union** Name Directives? UnionDefMemberTypes?
///
/// Detailed documentation can be found in [GraphQL spec](https://spec.graphql.org/October2021/#sec-UnionDef).
#[derive(Debug, Clone)]
pub struct UnionTypeDef {
    pub(crate) name: Name,
    pub(crate) description: Option<Description>,
    pub(crate) members: IndexSet<Name>,
    pub(crate) directives: IndexMap<Name, Directive>,
    pub(crate) extend: bool,
}

impl From<UnionTypeDef> for ast::Definition {
    fn from(x: UnionTypeDef) -> Self {
        if x.extend {
            ast::UnionTypeExtension {
                name: x.name.into(),
                directives: Directive::to_ast(x.directives),
                members: x.members.into_iter().map(Into::into).collect(),
            }
            .into()
        } else {
            ast::UnionTypeDefinition {
                description: x.description.map(Into::into),
                name: x.name.into(),
                directives: Directive::to_ast(x.directives),
                members: x.members.into_iter().map(Into::into).collect(),
            }
            .into()
        }
    }
}

impl TryFrom<apollo_parser::cst::UnionTypeDefinition> for UnionTypeDef {
    type Error = crate::FromError;

    fn try_from(union_def: apollo_parser::cst::UnionTypeDefinition) -> Result<Self, Self::Error> {
        Ok(Self {
            name: union_def
                .name()
                .expect("object type definition must have a name")
                .into(),
            description: union_def.description().map(Description::from),
            directives: union_def
                .directives()
                .map(Directive::convert_directives)
                .transpose()?
                .unwrap_or_default(),
            extend: false,
            members: union_def
                .union_member_types()
                .map(|members| {
                    members
                        .named_types()
                        .map(|n| n.name().unwrap().into())
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
}

impl TryFrom<apollo_parser::cst::UnionTypeExtension> for UnionTypeDef {
    type Error = crate::FromError;

    fn try_from(union_def: apollo_parser::cst::UnionTypeExtension) -> Result<Self, Self::Error> {
        Ok(Self {
            name: union_def
                .name()
                .expect("object type definition must have a name")
                .into(),
            description: None,
            directives: union_def
                .directives()
                .map(|d| {
                    d.directives()
                        .map(|d| Ok((d.name().unwrap().into(), Directive::try_from(d)?)))
                        .collect::<Result<_, crate::FromError>>()
                })
                .transpose()?
                .unwrap_or_default(),
            extend: true,
            members: union_def
                .union_member_types()
                .map(|members| {
                    members
                        .named_types()
                        .map(|n| n.name().unwrap().into())
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
}

impl DocumentBuilder<'_> {
    /// Create an arbitrary `UnionTypeDef`
    pub fn union_type_definition(&mut self) -> ArbitraryResult<UnionTypeDef> {
        let extend = !self.union_type_defs.is_empty() && self.u.arbitrary().unwrap_or(false);
        let name = if extend {
            let available_unions: Vec<&Name> = self
                .union_type_defs
                .iter()
                .filter_map(|union| {
                    if union.extend {
                        None
                    } else {
                        Some(&union.name)
                    }
                })
                .collect();
            (*self.u.choose(&available_unions)?).clone()
        } else {
            self.type_name()?
        };
        let description = self
            .u
            .arbitrary()
            .unwrap_or(false)
            .then(|| self.description())
            .transpose()?;
        let directives = self.directives(DirectiveLocation::Union)?;
        let extend = self.u.arbitrary().unwrap_or(false);
        let mut existing_types = self.list_existing_object_types();
        existing_types.extend(
            self.union_type_defs
                .iter()
                .map(|u| Ty::Named(u.name.clone())),
        );

        let members = (0..self.u.int_in_range(2..=10)?)
            .map(|_| Ok(self.choose_named_ty(&existing_types)?.name().clone()))
            .collect::<ArbitraryResult<IndexSet<_>>>()?;

        Ok(UnionTypeDef {
            name,
            description,
            members,
            directives,
            extend,
        })
    }
}
