# sloppyquorum.com

Source for [sloppyquorum.com](https://sloppyquorum.com), a Hugo
static site using the [hugo-texify2](https://github.com/weastur/hugo-texify2)
theme (vendored as a git submodule) and deployed to S3 +
CloudFront.

## Setup

Clone with the theme submodule:

```bash
git clone --recurse-submodules <repo-url>
# or, if already cloned:
git submodule update --init --recursive
```

Install Hugo extended >= 0.161 (the workflow pins 0.161.1):

```bash
# Linux, no sudo:
curl -sSL -o /tmp/hugo.deb \
  https://github.com/gohugoio/hugo/releases/download/v0.161.1/hugo_extended_0.161.1_linux-amd64.deb
dpkg-deb -x /tmp/hugo.deb ~/.local
# macOS:
brew install hugo
```

Install lychee 0.24.x for the link checker:

```bash
cargo install lychee
# or grab a binary from https://github.com/lycheeverse/lychee/releases
```

## Local development

```bash
hugo server
# http://localhost:1313/
```

**Never commit `public/` written by `hugo server`** -- it bakes
`http://localhost:1313/` into every URL. The `deploy` binary's
leak guard catches that.

## The `deploy` binary

`deploy/` is a Rust binary that wraps build, link check,
S3 upload, and CloudFront invalidation behind one command.
Subcommands run their prerequisites: `check` includes `build`,
`push` includes both.

```bash
# from the repo root
cargo run --release --manifest-path deploy/Cargo.toml -- <cmd>

# or once, then call `deploy` from anywhere:
cargo install --path deploy
```

| Subcommand | What it does                                                                 |
|------------|------------------------------------------------------------------------------|
| `build`    | `hugo --minify --gc --cleanDestinationDir`.                                  |
| `check`    | `build` + grep `public/` for `localhost:NNNN` leaks + lychee (offline).      |
| `push`     | `check` + parallel S3 PUT for every file in `public/` (sets `Content-Type` from extension) + optional prune of stale S3 keys + CloudFront `/*` invalidation on each distribution. |

`push` requires `--bucket` and `--distributions` (or
`DEPLOY_BUCKET` / `DEPLOY_DISTRIBUTIONS` env vars). AWS
credentials come from the default chain (env, `~/.aws/credentials`,
IMDS).

```bash
# typical deploy
deploy push --bucket sloppyquorum.com --distributions D111ABC,D222XYZ

# or
export DEPLOY_BUCKET=sloppyquorum.com
export DEPLOY_DISTRIBUTIONS=D111ABC,D222XYZ
deploy push

# rehearse without touching AWS
deploy push --bucket sloppyquorum.com --distributions D111ABC --dry-run
```

Flags worth knowing:

- `--bitwarden-item NAME`: pull AWS creds from a Bitwarden vault
  item instead of the default credential chain (see below).
- `--no-prune`: keep S3 keys that no longer exist in `public/`.
- `--concurrency N`: max parallel uploads (default 16).
- `--public-dir`, `--base-url`, `--repo`: override defaults.

### Pulling AWS credentials from Bitwarden

The `push` subcommand can fetch AWS credentials from a Bitwarden
vault entry, which is handy if you don't want long-lived keys
sitting in `~/.aws/credentials`. It shells out to the Bitwarden
CLI (`bw`), so install that first if you haven't:

```bash
# native binary, no Node required:
LATEST=$(curl -s https://api.github.com/repos/bitwarden/clients/releases \
  | grep -oE '"tag_name": "cli-v[0-9.]+"' | head -1 | grep -oE 'v[0-9.]+')
curl -sSL -o /tmp/bw.zip \
  "https://github.com/bitwarden/clients/releases/download/cli-$LATEST/bw-oss-linux-${LATEST#v}.zip"
unzip -o /tmp/bw.zip -d ~/.local/bin/ && chmod +x ~/.local/bin/bw
```

Store the AWS key as a **Login** item with this layout:

| Field            | Value                                  |
|------------------|----------------------------------------|
| Name             | anything; pass it as `--bitwarden-item`|
| Username         | `AWS_ACCESS_KEY_ID`                    |
| Password         | `AWS_SECRET_ACCESS_KEY`                |
| Custom field `AWS_SESSION_TOKEN` (optional) | STS session token, if used |

Then on each deploy session:

```bash
# one-time per login:
bw login

# one-time per shell session:
export BW_SESSION=$(bw unlock --raw)

# every deploy:
deploy push --bucket sloppyquorum.com \
            --distributions D111ABC,D222XYZ \
            --bitwarden-item aws-sloppyquorum

# or set it once in your shell rc:
export DEPLOY_BITWARDEN_ITEM=aws-sloppyquorum
deploy push --bucket sloppyquorum.com --distributions D111ABC,D222XYZ
```

When `--bitwarden-item` is set, the AWS SDK uses those credentials
exclusively; it does not fall through to env vars or
`~/.aws/credentials`. If `BW_SESSION` is missing or the vault
entry doesn't have the expected fields, `push` errors out before
making any AWS call.

CI runs the same link check on every push and PR via
`.github/workflows/check-links.yml`; a broken link shows a red X
on the commit. CI does not push to S3.

## Layout

| Path                                | What lives there                                                       |
|-------------------------------------|------------------------------------------------------------------------|
| `content/post/`                     | Posts (Markdown with TOML frontmatter).                                |
| `content/about.md`                  | About page.                                                            |
| `layouts/_default/`                 | Site-level overrides of theme templates (`baseof.html`, `rss.xml`).    |
| `static/`                           | Favicons, manifest, `css/custom.css`. Shadows theme `static/` by path. |
| `themes/hugo-texify2/`              | Vendored theme (submodule, do not edit).                               |
| `public/`                           | Generated output. Committed so deploys can sync straight from a clone. |
| `deploy/`                   | Rust binary (`build` / `check` / `push`).                              |
| `lychee.toml`                       | Lychee config (offline-friendly defaults, fragment checking).          |
| `.github/workflows/check-links.yml` | CI: builds and link-checks every push / PR.                            |

## Writing a new post

```bash
hugo new post/<slug>.md
```

Frontmatter template is in `archetypes/default.md`. The
`date` field controls listing order; `tags` map to `/tags/<tag>/`
pages and feeds.
