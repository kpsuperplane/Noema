# Personal assistant replay inputs

[manifest.json](manifest.json) is an input to
[the replay script](../../../../scripts/acceptance/personal-assistant-replay.ts).
The script reads the 100-case table in
[the assessment](../../../difficult-digital-personal-assistant-tasks.md).
Keep its table format and case numbers compatible with that parser.

The manifest supplies synthetic prompts and a local fixture address.
It does not record successful acceptance or prove real-provider behavior.

Without `--execute`, the script checks inputs and prints selected cases without running them:

```sh
bun run scripts/acceptance/personal-assistant-replay.ts --limit 2
```

Execution requires the mock service and a working live runner.
The script marks a successful runner exit as `completed`.
Review the returned evidence before treating that state as a passed acceptance case.
