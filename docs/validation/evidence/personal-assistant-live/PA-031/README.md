# PA-031 — Purchase comparison

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through `/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic products API, fixture `2026-09-08-products-api-v1`.
- Documentation: `https://retreat-barcelona-less-functionality.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile` and `/v1/products`.
- The service is read-only. It has no retailer, payment, shipping, or real account.
- Fixture source: `scripts/acceptance/run-mock-products-api.ts`.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Connection: `d8941cb8c33d9765572ca0795af559d1`.
- Final accepted definition: revision `v5`, digest
  `c7dd81101a4be977ee1192d86b8b278dc5a861c3f4f048c0ce5e713d84b48732`.
- Final connection state: active, connection revision 6, policy revision 2,
  with only `get_profile` and `list_products` allowed.

## Connector setup and repairs

Noema first read the service documentation, proposed a bounded read-only API
definition, and paused for review. The reviewed revisions were accepted in
order. Each revision was tested through a fresh connector read before the
case advanced.

| Revision | Accepted definition digest | Reason for revision |
| --- | --- | --- |
| v1 | `33f1d7e3ff35efd07028138ddbd8871f80cbfa3cfa8ab54d2721bc6033a972df` | Initial profile and listing reads. |
| v2 | `14d513d0112d5622797da10adc25d5a0b4664379c9461626e55d5d4909c80bf5` | Read nested profile and listing compatibility objects. |
| v3 | `74706550bb7d9008100a445fd8eab0277b65191e6aa4e84ac286f99d2eddd97b` | Preserve explicit `false` Linux and USB-C values. |
| v4 | `825ea25a591bf42fdc2f4b8b8ded023dbff7b3a1839dcefdbc0d3ab6b7902eec` | Preserve documented `warranty_years`. |
| v5 | `c7dd81101a4be977ee1192d86b8b278dc5a861c3f4f048c0ce5e713d84b48732` | Final accepted revision after the v4 proposal. |

The first proposal digest was `1b5aa1a574798b2b483d1a1b1c789618ceac45ef18c7d317fe4c509c76def37b`.
The subsequent pending revision digests were
`a9f587a39c54b2a25c07e35e7af2e48bd3a13b47e0ca03d7dd72f89b92d76b4a`,
`5b7d246d7c17329bd21576b0e5e8158ca500d0c19c23532b129dcda4faac4130`,
`2237ff37da94df5fc595bcd80f7aa96acd194c7f9fe5353dbfb8dd6d7b4d57b6`, and
`4f8a5ff490c58d0e7cb23e6051f1b71a1ac08d881d67188d859fc24d7f6151d4`.

## Fresh reads

Turn `turn:5b4d78a1560c0039601f89bd4d12a6a8` read the profile and all three
listings after revision v5. The connector returned the complete required
profile and every listing field needed for the decision:

- Budget: `$600 USD`.
- Required compatibility: Linux `true`, minimum memory `16 GB`, USB-C charging
  `true`.
- CloudBook Air: `$560` price, `$25` shipping, `$45` tax, one-year warranty,
  three-day delivery, compatible.
- FieldBook: `$575` price, `$0` shipping, `$18` tax, two-year warranty,
  five-day delivery, compatible.
- StudioBook: `$590` price, `$0` shipping, `$10` tax, three-year warranty,
  two-day delivery, Linux `false`, 32 GB, USB-C charging `false`.

The tool results were recorded in `conversation_item:2161` and
`conversation_item:2163`. No purchase operation was available or called.

## Comparison and saved result

Turn `turn:a1b57788d8e96511405f25489a27a257` computed each landed cost as
price plus shipping plus tax and selected FieldBook.

| Product | Landed cost | Compatibility | Budget result | Decision |
| --- | ---: | --- | --- | --- |
| CloudBook Air | `$630` | Meets all requirements | `$30` over | Rejected |
| FieldBook | `$593` | Meets all requirements | `$7` under | Recommended |
| StudioBook | `$600` | Fails Linux and USB-C requirements | Within budget | Rejected |

The response cited all three listing URLs and stated that no purchase or order
was made. Noema saved the recommendation as artifact
`artifact:cea50d92a04f5fb218e23445c463fe82`, version
`artifact_version:6180d7cb0f82ec602c134d054dc22a85`, titled
`Laptop recommendation — September 8, 2026`.

The saved artifact includes the calculations, constraints, source URLs, and
uncertainty about unlisted costs, future listing changes, compatibility claims,
and warranty terms.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read service documentation before connecting | Pass. Documentation was fetched before the first proposal. |
| Use a reviewed, read-only connection | Pass. The accepted connection exposes only two GET operations. |
| Return all profile constraints | Pass. Budget, Linux, memory, and USB-C requirements arrived in the fresh read. |
| Return all listing fields | Pass. Prices, shipping, tax, warranty, delivery, source URLs, and compatibility values arrived, including explicit `false` values. |
| Compute landed costs | Pass. All three sums are shown and independently match the fixture. |
| Reject over-budget and incompatible products | Pass. CloudBook Air and StudioBook were rejected for the correct reasons. |
| Apply the stated preference | Pass. FieldBook was selected as the only compatible option under budget, with a longer warranty and five-day delivery. |
| Cite current mock listings | Pass. All three source URLs were included in the response and saved artifact. |
| Save recommendation and uncertainty | Pass. The local recommendation artifact was created and named in the response. |
| Avoid real side effects | Pass. The fixture has no write operation, and Noema made no purchase or order. |
