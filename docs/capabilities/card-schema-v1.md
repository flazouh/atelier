# Tool card schema, v1 (draft for review)

Status: draft. Docs only. Section 8.1 of `tasks-v1.md` says when a plugin uses a card schema. This file says what the schema is.

## 1. What it is

A card schema is a JSON file that a plugin ships for one of its tools. It tells Atelier how to draw the result of that tool call.
It is data. It holds no script, no HTML and no style. Atelier draws the card with its own components, so the card looks native in the
dark theme, the light theme and at every zoom level. The iOS app reads the same file and draws it with its own components.

Files in this folder:
- `card.schema.json`: the JSON Schema of a card (checked: valid draft 2020-12).
- `sentry.search_issues.card.json` and `sentry.get_issue.card.json`: two real cards (checked against the schema).
- `sentry.sample-result.json`: a sample result of `sentry.search_issues`.

## 2. Shape

A card has up to three states: `running`, `failed` and `done`. The first two have only a title. `done` has a `title`, a `body`, and
a `footer` of up to three actions. The header (logo, plugin name, account, origin) comes from the manifest and the host. A card does not draw it.

A body is a tree of **nodes**. The set is closed: `stack`, `text`, `badge`, `metric`, `icon`, `avatar`, `code`, `list`, `button`, `divider`.
A new node type needs a pull request to Atelier. A plugin cannot add one.

## 3. Values

A value is a literal, a **path** into the tool result (`{ "path": "$.title" }`), or a **template** (`"{$.project} · last seen {$.last_seen|relative_time}"`). A hole may name a format after a bar.
A path may carry a **format**: `count` (1.2k), `relative_time`, `absolute_time`, `duration_ms`, `bytes`, `percent`.
Nothing else is evaluated: no expression, no function, no loop outside `list`. A node may carry `when`: `{ path, equals }` or `{ path, exists }`.

## 4. Actions

- `open`: opens an entity by `ref` in its shared screen in Atelier.
- `open_url`: opens an https link in the system browser. The host shows the domain first.
- `call`: calls another tool of the same plugin with arguments from paths. A write tool still asks for approval, as always.

## 5. Look

- Colour comes only from a **tone**: `neutral, info, success, warning, danger`. The theme maps a tone to a colour. A card cannot set a raw colour.
- Icons come from the Atelier icon set by name. An unknown name shows a default icon.
- An avatar fetches its image through the host, without cookies, and only from origins that the manifest lists.
- Limits: 12 children per stack, depth 6, 200 nodes in total, 50 rows per list (more rows show "Show more"), 6 lines of text, 30 lines of code.

## 6. Example: Sentry

A plugin without a capability uses `plugin` as the first part of its references: `plugin:sentry:acme:WEB-1A`.

The manifest lists each tool, its permission class and its card:

```json
{
  "id": "sentry",
  "name": "Sentry",
  "logo": "logo.svg",
  "auth": ["oauth"],
  "tools": [
    { "name": "sentry.search_issues", "permission": "read",  "card": "sentry.search_issues.card.json" },
    { "name": "sentry.get_issue",     "permission": "read",  "card": "sentry.get_issue.card.json" },
    { "name": "sentry.resolve_issue", "permission": "write", "card": null }
  ]
}
```

`sentry.search_issues` with the sample result draws one row per issue: a red bug icon for an `error`, a title on one line, a grey line
"web · last seen 3 hours ago", a badge "142 events", and a yellow badge "regressed" when the issue regressed. A click on a row opens it.
`sentry.get_issue` draws the status badges, three metrics, the culprit, a stack excerpt, and the footer actions: Open in Atelier, Open in Sentry, Resolve.
`sentry.resolve_issue` has no card, so it gets the generic card. It is a write tool, so it asks for approval first.

## 7. Safety and failure

1. Atelier checks every card against `card.schema.json` when it installs the plugin. A bad card is refused with the error.
2. A path that finds nothing shows the `default`, or nothing. The card never fails because of data.
3. A result that is not the shape the card expects falls back to the generic card.
4. A tool result is untrusted text. The card draws it as text, never as markup.

## 8. Tests

1. Every card in the repo passes the schema.
2. A golden render of each example card with its sample result, in dark, light and at 150 percent zoom. The test compares pixels to a stored image.
3. A card with a missing path draws without a crash.
4. The Rust renderer and the Swift renderer give the same node tree for the same card and result.

## 9. Open questions

1. **Markdown in text.** Allow bold and links in `text` (many issue titles and messages have them), or plain text only?
2. **Images.** Allow a screenshot node (Sentry has none, but PostHog and Linear do), with the same origin rule as avatars?
3. **Who owns promotion.** When a second error-tracking provider arrives, who writes the capability: the Sentry plugin author or Atelier?
