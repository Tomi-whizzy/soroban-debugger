# Session Bookmarks

Session bookmarks allow you to mark important execution states during an interactive, TUI, or REPL debugging session. Instead of stepping back manually or restarting the execution from scratch, you can assign a label to a specific paused state and instantly jump back to it later in the same session.

## Creating a Bookmark

While paused in an interactive session, use the `bookmark` (or `bm`) command with a label:

```bash
(debug) bookmark "before_auth"
> Bookmark 'before_auth' created at step 42.
```

## Listing Bookmarks

To see all bookmarks created in the current session:

```bash
(debug) bookmarks
> Active Bookmarks:
  1. before_auth (Step 42)
  2. after_storage_write (Step 105)
```

## Jumping to a Bookmark

To rewind execution to a previously bookmarked state, use the `jump` command with the label:

```bash
(debug) jump "before_auth"
> Rewound to bookmark 'before_auth' (Step 42).
```

## Under the Hood

Bookmarks record the exact execution step (instruction offset or call sequence index) and the session identifier. The underlying time-travel/replay mechanism handles restoring the state of the Soroban environment, including memory and storage, to match the exact moment the bookmark was created.