# Vendored A2UI authority

These files pin the official A2UI v0.9.1 protocol and Basic catalog used to
define Noema's smaller, locally validated catalog. They were copied from
[`a2ui-project/a2ui`](https://github.com/a2ui-project/a2ui) at commit
`ef941afd93267a2218f5aaca1fcc27da87f0e464`. Runtime validation never fetches
schema authority from the network.

Noema advertises only the component subset declared in
`src/a2ui/authority.rs`; vendoring the upstream Basic catalog does not imply
that every upstream component is supported.
