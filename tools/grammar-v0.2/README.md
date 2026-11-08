# v0.2 grammar check

This is a standalone Phase 0 check crate. It is not a workspace member, is not
shipped, and is not a compiler dependency.

It encodes the v0.2 ambiguity set as a canonical LR(1) grammar:

- `fail Type(...)` versus `fail Type { ... }` versus ordinary calls;
- turbofish `::<...>` and type arguments in type position;
- struct initializers versus block syntax;
- both scope-block spellings;
- optional trailing commas in every comma-separated list.

`qana-grammar` builds canonical LR(1) tables with Pager merging and reports any
surviving conflict with items and an example prefix. The crate is green only
when the grammar has zero conflicts and the checker still rejects a known
dangling-else grammar.

Run:

```text
cargo test
```

The focused grammar omits operator-precedence layers and control-flow
expressions; they do not participate in the ambiguity set. It also models one
postfix suffix per expression; chained postfix forms are covered by the parser
suite and `Docs/spec-v0.2.md` section 3.9.
