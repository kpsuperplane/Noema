# Chat history preparation recovery

The failed turns at 19:52–19:54 UTC on September 9 stopped during `Prepare model context`.
The server remained reachable. The saved error contained no underlying cause.
A separate provider request successfully summarized the same completed history.
Saving a summary also passed against an isolated database copy.
These checks do not prove the exact cause of the original failures.

An optional summary failure previously stopped Chat even when its history still fit the model limit.
The runtime now retains that history and continues. Oversized history still requires successful summarization.
Cancellation still stops the request. No live history was changed during diagnosis.

The deterministic regression test covers fitting and oversized history with a failed summary request.
Focused context tests passed. Broad Go tests passed except the unrelated `TestTmpPA069Validate` temporary adapter test.
The first parallel vet run lost a compiler process. Vet passed when retried with one package at a time.

The patch adds three production Go lines and 34 test Go lines. Generated GraphQL is unchanged.
The inclusive Go addition is 37 lines. No schema migration was needed.
