# Glossary

lathe's own words, and the ones it keeps from GitQuiet. Code uses the same names.

## The app

- **Project** (`Project`): a folder lathe works in, wherever it lives. Everything lathe does to its
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

- **Family**: the themes that are one design in light and dark, such as lathe or Catppuccin.
- **Raise**: move a colour's lightness until it reaches a contrast target against the page: 4.5:1
  for text, 3:1 for marks.
- **Mark**: a status colour on an icon or a dot, not on text, held to 3:1.
