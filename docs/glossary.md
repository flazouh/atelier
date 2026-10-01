# Glossary

atelier's own words, and the ones it keeps from GitQuiet. Code uses the same names.

## The app

- **Project** (`Project`): a folder atelier works in, wherever it lives. Everything atelier does to its
  files, processes and git goes through this one interface.
- **Local project** (`LocalProject`): a project in a folder on this machine. A remote project (M1b)
  lives on an SSH host.
- **Location** (`Location`): where a project lives, as the recent list keeps it: a local path, or a
  host and a path.
- **Open project** (`OpenProject`): a project open in a window, with its tree, tabs, buffers and
  language servers.
- **Buffer**: an open file's text in its tab. **Dirty**: it holds edits not yet saved.
- **Changed on disk**: the file changed under a dirty buffer. The tab keeps its edits and asks.

## Merging

- **Merge facts** (`MergeFacts`): everything the app knows about merging one pull request, as the
  forge reports it: its state, the blockers, the methods the repository allows, merge when ready,
  the branch setting and the reader's rights.
- **Method** (`MergeMethod`): how the pull request lands: "Create a merge commit", "Squash and
  merge" or "Rebase and merge".
- **Blocker** (`Blocker`): one thing that holds a merge up. They come in a fixed order: a draft,
  conflicts, a branch behind its base, required checks failing, then running, the review, and a
  merge queue in force. The first one decides the button.
- **Standing** (`standing`): the one line that says where the merge stands, such as "Ready to
  merge", "2 checks still running" or "Blocked: changes asked by Ada".
- **Choice** (`Choice`): what the reader picked in the merge menu: the method, merge when ready,
  and "Delete branch after merging".
- **Remembered method**: the method the reader used last in a repository. The app keeps it; the
  button reports each new choice.
- **Rights** (`Rights`): whether the reader can merge, can bypass the rules as an administrator, or
  cannot merge.
- **Merge when ready**: the forge merges by itself once the blockers that can wait (checks and
  reviews) clear. Other forges call it auto-merge.
- **Merge queue**: the repository lands pull requests one at a time through a queue; the button
  adds to it rather than merging.

## Themes

- **Family**: the themes that are one design in light and dark, such as atelier or Catppuccin.
- **Raise**: move a colour's lightness until it reaches a contrast target against the page: 4.5:1
  for text, 3:1 for marks.
- **Mark**: a status colour on an icon or a dot, not on text, held to 3:1.

## Review

- **Turn**: one message and everything the agent did for it, up to its end. A message sent while a turn
  runs joins it.
- **Turn tracker** (`TurnTracker`): takes each file's text before the agent's first touch in a turn,
  and the turn's changed files at its end.
- **Changed files card**: the list of a turn's changed files, with `+a -r`, after the turn's last row.
- **Review pane** (`ReviewPane`): the review of one turn, or of the whole session (its **scope**), in
  place of the editor.
- **Merged**: one file under review: both versions in one text, each hunk its old rows then its new ones.
- **Decided**: a hunk accepted or rejected. The pane keeps each file as the reader left it, per scope.
- **Reviewed** (`x`): the reader's mark on a file of a turn. A file with no hunk left counts as reviewed.
- **Answered**: a comment sent with a message, once the agent's turn after it ended; it shows Resolved.
