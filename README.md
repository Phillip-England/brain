# brain

`brain` is a single-user Rust web app for saving voice-created markdown ideas into project folders.

## Install

```sh
cargo install --path .
```

## Credentials

Set the admin login with the CLI:

```sh
brain credentials set admin change-this
```

This writes `credentials.json` inside the app home (`BRAIN_HOME`, or `~/.brain` by default), so the web server can read the same credentials no matter which directory the binary is started from.

Environment variables are still supported and take priority over the credentials file:

```sh
export BRAIN_ADMIN_USERNAME='admin'
export BRAIN_ADMIN_PASSWORD='change-this'
```

Check whether credentials are available:

```sh
brain credentials status
```

## Run

```sh
brain
```

The default URL is `http://127.0.0.1:8787`.

Optional environment variables:

```sh
export BRAIN_ADDR='127.0.0.1:8787'
export BRAIN_HOME="$HOME/.brain"
```

`BRAIN_HOME` is the app home and initial brain directory. The admin settings page can change the brain directory later.

## Markdown Rules

An idea must start with a single `# Title` header. After that it may contain markdown headers and normal paragraphs.

Accepted:

```md
# Server

The server stores markdown files.

## API

The API saves ideas into project folders.
```

Rejected:

```md
## Missing top-level title

- Lists are not paragraphs.
```

## Login Abuse Protection

Bad login attempts are recorded in SQLite. On every login attempt, entries older than 24 hours are deleted. An IP address is blocked after 5 failed attempts inside the remaining 24-hour window.

## Copying And Exporting

The Ideas page can copy or export:

- One selected idea
- The current search results
- Every idea in the brain system

The Projects page can copy or export every idea inside a single project. Exports download as markdown files, and bulk exports separate ideas with `---` plus a short HTML comment containing the project, creation time, and idea id.

## Recording Controls

The Controls page in the app lists the recorder shortcuts. The most important ones are:

- `Enter` starts recording
- `\` stops recording
- `S` saves the current idea and clears the board
- `1`, `2`, `3`, `4` switch heading levels while recording
- `P` switches to paragraph mode while recording

Project switching happens on the Projects page by clicking a project.
