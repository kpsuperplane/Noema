# GraphQL operations

Apollo iOS operation documents belong in this directory. The code generation
configuration reads the canonical schema from `../../graphql/schema.graphql`
and packages generated `NoemaAPI` sources in `Noema/Generated`.

No native operations are checked in yet; the first operation slice should add a
`.graphql` document here and regenerate the adjacent generated output.
