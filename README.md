# srv

Serve a file or directory over HTTP. Directory listings are browsable, Markdown files render as HTML, and `--docs` opens a browser for a tree of Markdown files. Mermaid flowchart code blocks in Markdown files render as inline SVG, offline and without JavaScript; any other Mermaid diagram type, or a diagram that fails to parse, is shown as a plain code block.

![srv directory listing](.github/screenshot.png)

## Install

Download the archive for your platform from [GitHub releases](https://github.com/laputa-systems/srv/releases). The archives are `srv-aarch64-apple-darwin.zip`, `srv-aarch64-unknown-linux-musl-static.zip`, and `srv-x86_64-unknown-linux-musl-static.zip`. On x86_64 Linux:

```sh
unzip srv-x86_64-unknown-linux-musl-static.zip
mkdir -p ~/.local/bin
install -m 755 srv-x86_64-unknown-linux-musl-static ~/.local/bin/srv
```

Put `~/.local/bin` on your `PATH`. Linux archives are statically linked; on macOS, unzip `srv-aarch64-apple-darwin.zip` and install `srv-aarch64-apple-darwin` the same way.

## Use

```sh
srv [options] [path]
srv ./public
srv -p 8080 -b 127.0.0.1 ./public
srv --docs ./docs
```

With no path, `srv` serves the current directory at `http://127.0.0.1:8000`.

| Option | Effect |
| --- | --- |
| `path` | File or directory to serve; defaults to `.`. A selected file is returned for any URL path. |
| `-p` | Pass a port number; defaults to `8000`. |
| `-b` | Pass a bind address; defaults to `127.0.0.1`. |
| `--docs` | Browse Markdown files under the selected root (a selected file uses its parent directory); hidden directories and directories matched by simple `.gitignore` patterns are skipped. |
| `-q` | Suppress per-request logs. |
| `-h`, `--help` | Print the complete reference and examples to standard output. |
| `-V`, `--version` | Print the version. |

For a directory, `index.html` is served when present; otherwise `srv` shows a listing. `.md` and `.markdown` files render as HTML. Mermaid `flowchart` blocks render as inline SVG on standalone Markdown pages; the `--docs` viewer and unsupported Mermaid types show a code block instead. Only GET requests are served. `srv` writes startup, request, and error messages to standard error; `-q` suppresses request messages.

## Onefile archives

If the file argument is an HTML document with a ZIP archive appended — the `.u.zip.html` format that the onefile capture extension produces — `srv` detects the embedded archive and serves the page at `/` and each entry at its archive path (for example `/resources/0.woff2`), with the MIME types recorded in the archive's `metadata.json`. `/` answers with the file whole because the archive's loader fetches the page's own URL to recover the entries; any other unknown path returns 404.

Path traversal using `..` is rejected, but symlinked directories under the selected path can expose files outside it. Serve only directories whose symlinks you trust.

For help, use `srv --help` or [open an issue](https://github.com/laputa-systems/srv/issues). Licensed under [MIT](LICENSE).
