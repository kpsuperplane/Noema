# GraphQL operations

Apollo iOS operation documents belong in this directory. The code generation
configuration reads the canonical schema from `../../graphql/schema.graphql`
and packages generated `NoemaAPI` sources in `Noema/Generated`.

Native operations are checked in by feature. After adding or changing a
document, regenerate the adjacent generated output on macOS and keep the
generated package in sync with the shared schema.
