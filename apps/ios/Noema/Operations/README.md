# GraphQL operations

Apollo iOS operation documents belong in this directory. The code generation
configuration reads the source schema from `../../graphql/schema.graphql`.
This path is relative to `apps/ios`, where the generation command runs.
The configuration packages generated `NoemaAPI` sources in `Noema/Generated`.

Native operations are checked in by feature. After adding or changing a
document, regenerate the adjacent generated output on macOS and keep the
generated package in sync with the shared schema.
