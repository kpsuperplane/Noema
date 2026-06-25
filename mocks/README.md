# Noema Chat-First IA Static Mocks

This folder contains static HTML/CSS/JS mocks for the revised frontend IA. The
primary experience is a single chat pane that incrementally reveals memory,
threads, workspaces, and settings drill-ins.

Open `index.html` directly or use hash routes such as `index.html#memory-expanded`.

## Screens

- `#first-chat`: first-run single chat pane.
- `#memory-event`: saved memory appears as a line item in chat history.
- `#memory-expanded`: clicking the line item opens inline memory details.
- `#threads`: multiple chat threads appear after there is more than one thread.
- `#work-reveal`: a chat creates a workspace/task panel beside the transcript.
- `#workspace`: task/project/workspace management grown from the chat.
- `#memory-settings`: secondary memory settings page reached from the inline memory card.
- `#advanced-admin`: local admin and inspection as a secondary settings area.

## Screenshot Capture

Run:

```sh
node mocks/capture.js
```

The script uses the local Google Chrome app in headless mode and writes PNGs to
`mocks/screenshots/chat-first/`.

For a narrow viewport spot check, run:

```sh
node mocks/capture.js --mobile --screens=first-chat,memory-expanded,threads,work-reveal
```
