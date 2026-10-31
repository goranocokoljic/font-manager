# AI assistants

An AI assistant such as Claude Code or Claude Desktop can use Font Manager through the
Model Context Protocol (MCP). MCP is a standard way for an assistant to call tools in a
program on your computer. The assistant can search your library, read font details, check
language coverage, render previews and pick fonts for a brief. It can change nothing until
you allow it, tool by tool.

Everything happens on this computer. Font Manager answers only while it runs, only on
this computer, and only to a program that knows the token of the local API. Font Manager
makes no network calls of its own for this (see [Getting started](getting-started.md)).

## Turn it on

1. Open Settings → **AI assistants**.
2. Turn on **Allow AI assistants (MCP server)**. It is off until you do.
3. Connect your assistant with one of the snippets below the switch. The snippets contain
   the token of the local API. Keep them secret, like a password.

Turn the switch off to stop all assistants at once. A request that an assistant sends
while the switch is off gets this answer: "Open Font Manager and turn on AI assistants in
Settings."

## Connect Claude Code

Claude Code talks to Font Manager over HTTP, on the address that Settings shows.

1. In Settings → AI assistants, click **Copy Claude Code command**.
2. Paste the command into a terminal and run it. It looks like this:

   ```
   claude mcp add --transport http font-manager http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"
   ```

3. Start Claude Code. Type `/mcp` to see that `font-manager` is connected, then ask it,
   for example, to find a serif family with Cyrillic.

The address changes each time Font Manager starts, so this connection stops working after
a restart of Font Manager. Then remove the old entry with `claude mcp remove font-manager`,
copy the command again, and run it again.

## Connect Claude Desktop

Claude Desktop cannot call an HTTP address of its own. It starts a small program instead,
`fm-mcp-stdio`, which comes with Font Manager. That program finds the running Font Manager
and its token itself. So the Claude Desktop snippet holds no token and no port, and it
keeps working after a restart of Font Manager.

1. In Settings → AI assistants, click **Copy Claude Desktop config**.
2. In Claude Desktop, open **Settings → Developer → Edit Config**. This opens
   `claude_desktop_config.json` in a file manager.
3. Open the file in a text editor and paste the `font-manager` entry into the `mcpServers`
   block. If the file is empty, paste the whole snippet. Save the file.
4. Quit Claude Desktop fully, also from the tray or the menu bar, and start it again.
5. In a new chat, the tools icon lists `font-manager`.

If Font Manager is not running when Claude Desktop starts, the tools are listed but every
call answers "Open Font Manager and turn on AI assistants in Settings." Start Font
Manager and ask again. Claude Desktop does not have to restart.

Other assistants that read the same kind of file (an `mcpServers` block with a command
and arguments) can use the same entry.

## Other assistants

For an assistant that connects over HTTP with a JSON config, click **Copy HTTP client
config**. The block names the address and the `Authorization` header with the token.

## What an assistant can do

Searching and reading are always allowed when the switch is on:

| Tool | What it does |
|------|--------------|
| `search_fonts`, `get_font` | Find families and read the details of a face |
| `list_collections`, `list_sets` | List your collections and project sets |
| `check_coverage`, `find_similar`, `get_pairings`, `get_license` | Language coverage, similar fonts, pairings, the license of a family |
| `render_preview`, `compare_preview` | Render a text in one face, or in several faces side by side, as an image |
| `pick_fonts_for_brief` | Read a brief such as "a calm bilingual brand" and suggest up to 5 families, with a reason for each |

The assistant can also read a font, a collection or a set as a resource (`font://<id>`,
`collection://<id>`, `set://<id>`), and use two ready prompts: **Choose fonts for a
brief** and **Check language coverage**.

Tools that change something each have their own switch under **What AI assistants may
change**. All are off until you turn them on. A change applies to the next request.

| Switch | Tools it allows |
|--------|-----------------|
| **Activate fonts** | `activate_fonts`, and the activation step of `prepare_for_web` |
| **Deactivate fonts** | `deactivate_fonts` (asks you every time, see below) |
| **Tag fonts** | `tag_fonts` |
| **Export web fonts** | `export_web`, `build_specimen`, and the export step of `prepare_for_web` |

When an assistant uses a tool that is off, nothing changes, and the assistant gets a
message that names the switch, for example: "AI assistants may not use deactivate_fonts
yet. Turn on "Deactivate fonts" in Settings → AI assistants to allow it." Turn the switch
on and tell the assistant to try again.

## Allow or deny a deactivation

Deactivating a font can break a document that is open in another program. So
**Deactivate fonts** asks you every time, also when its switch is on. A message shows at
the bottom of the Font Manager window:

> An AI assistant wants to deactivate 1 font. No answer in 60 seconds counts as Deny.

Click **Allow** to deactivate the fonts, or **Deny** to keep them active. If you do not
answer in 60 seconds, it counts as Deny, and nothing changes. The assistant gets the
result either way.

## Keep it safe

- The switch is off after every install. Nothing can reach Font Manager until you turn it
  on.
- Every assistant on this computer shares the same switches. There is no switch per
  assistant.
- Font Manager never deactivates a font that it did not activate, also when an assistant
  asks for it.
- A snippet with the token works until Font Manager makes a new token. To make a new
  token, open Settings → **Local API** and click **Regenerate**. Then copy the
  snippets again.

Next: [Keyboard shortcuts](keyboard-shortcuts.md).
