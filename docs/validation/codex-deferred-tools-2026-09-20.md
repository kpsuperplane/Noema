# Codex deferred tool spot test

The live Codex endpoint supports hosted deferred tool loading for
`gpt-5.6-terra` on the development account tested on 2026-09-20.

The probe used `https://chatgpt.com/backend-api/codex/responses`, HTTP streaming,
`store: false`, low reasoning effort, one synthetic shipping namespace, one
function with `defer_loading: true`, and `{"type":"tool_search"}`.
Credentials passed directly from the protected development store into request
headers. No credentials entered output files. No Chat message or real connector
action was created.

| Case | Observed result | Input tokens | Elapsed |
| --- | --- | ---: | ---: |
| Shipping lookup | `tool_search_call`, `tool_search_output`, then `get_shipping_eta` with `order_id: "order_42"` | 600 | 3.424 s |
| Casual reply with neutral instructions | Returned `yum`; no search or function call | 419 | 4.012 s |

Both requests returned HTTP 200 and completed. The synthetic function was not
executed. This proves discovery and function-call emission, not the full tool
result cycle, WebSocket continuation, or savings with a large real catalog.
These single observations are not a latency benchmark.

The first shipping attempt needed a capture correction: completed output arrived
through streamed item events. The second attempt captured those events.
The first casual request inherited an instruction to use tools and loaded the
namespace unnecessarily. Neutral instructions removed that conflict and passed.
Four provider requests were sent in total.

The probe and exact synthetic requests and results remain under
`/var/tmp/noema-deferred-20260920/`. No production code changed.

Reference: [OpenAI tool search](https://developers.openai.com/api/docs/guides/tools-tool-search).
