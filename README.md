<div align="center">
  <h1><code>apollo-rs</code></h1>

  <p>
    <strong>Rust tooling for manipulation of the GraphQL language.</strong>
  </p>
</div>

## Tools included

This project is intended to house a number of tools related to the low-level
workings of GraphQL according to the [GraphQL specification]. Nothing in
these libraries is specific to Apollo, and can freely be used by other
projects which need standards-compliant GraphQL tooling written in Rust. The
following crates currently exist:

* [**`apollo-compiler`**](crates/apollo-compiler/) - a library to manipulate, semantically analyze, and validate GraphQL schema definition and query language
* [**`apollo-parser`**](crates/apollo-parser) - a library to parse the GraphQL (used by `apollo-compiler`)
* [**`apollo-smith`**](crates/apollo-smith) - a test case generator to deterministically produce arbitrary GraphQL documents

Please check out their respective READMEs for usage examples.

## Status
`apollo-rs` is a living project that keeps evolving and is being used in production.
If you try out `apollo-rs` and run into trouble, we encourage you to open an [issue].

## Design Principles
1. **Prioritizing developer experience.** Elegant and ergonomic APIs is the
theme for Rust as a language, and we want to make sure that all component APIs
we provide are aligned with these principles.

2. **Stability and reliability.** Spec-compliant, and idempotent APIs
which can be used safely in enterprise-grade codebases.

3. **Diagnostics.** The tools are to be written in a way that will allow us to
produce detailed diagnostics. It does not panic or return early if there is a
lexical or a syntactic error. Instead, the parser is meant to gather as much
context and information as possible and return errors alongside the output that
is valid. Coincidentally, this allows for easily debuggable code for those
maintaining this project.

4. **Extensibility.** The parser is written to work with different use cases in
our budding Rust GraphQL ecosystem, be it building schema-diagnostics for Rover,
or writing out query planning and composition algorithms in Rust. These all have
quite different requirements when it comes to document manipulation. We wanted to
make sure we account for them early on.

## Rust versions

`apollo-rs` is tested on the latest stable version of Rust.
Older version may or may not be compatible.

## License
Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

[issue]: https://github.com/apollographql/apollo-rs/issues/new/choose
[GraphQL specification]: https://spec.graphql.org/October2021