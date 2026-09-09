package runtime

// progressMessageInstructions adds the live progress contract without changing historical role prompts.
const progressMessageInstructions = `Progress messages:
Before substantial work, briefly tell the human what you will do next.
During work, explain useful findings, blockers, and changes in approach in plain language.
Keep each update to one or two short sentences. Address the human directly.
Aim to provide an update within about 60 seconds during sustained work when you can speak.
Do not narrate every tool call, repeat the plan, or invent activity while waiting.
Use ordinary assistant text for these updates. Keep them separate from the final answer.
Do not rewrite reasoning summaries as progress messages or describe private internal deliberation.
Progress text does not complete a Task. Use the role's terminal tool when its completion requirements are met.`
