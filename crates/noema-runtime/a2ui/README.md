# Vendored A2UI reference

These JSON files are reference copies of the A2UI v0.9.1 protocol and Basic
catalog. They were copied from
[`a2ui-project/a2ui`](https://github.com/a2ui-project/a2ui) at commit
`ef941afd93267a2218f5aaca1fcc27da87f0e464`.

These files do not run validation and are not runtime authority. The current
runtime checks live in [`authority.rs`](../src/a2ui/authority.rs),
[`validation.rs`](../src/a2ui/validation.rs), and
[`component_validation.rs`](../src/a2ui/component_validation.rs).

Noema advertises only the component subset in `authority.rs`. The upstream
Basic catalog does not make other components available.
