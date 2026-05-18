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

## Local development

```bash
hugo server
# http://localhost:1313/
```

## Build

```bash
hugo --minify --gc --cleanDestinationDir
```

This regenerates `public/` against the production `baseURL` in
`hugo.toml`. **Never check in `public/` that was written by
`hugo server`** -- it bakes `http://localhost:1313/` into every
URL. The link checker below catches that.

## Link check

Verify every internal link and anchor in the built site:

```bash
./scripts/check-links.sh           # offline (internal only, fast)
./scripts/check-links.sh --online  # also hit external URLs
```

The script (1) rebuilds, (2) greps `public/` for `localhost:NNNN`
leaks and fails loudly if any are found, and (3) runs lychee
(`lychee.toml`) with `--root-dir` plus a
`https://sloppyquorum.com/ -> file://.../public/` remap so every
internal href resolves against the local build.

Same checks run in CI on every push and PR via
`.github/workflows/check-links.yml`; a broken link shows a red X
on the commit.

Install lychee 0.24.x locally:

```bash
cargo install lychee
# or grab a binary from https://github.com/lycheeverse/lychee/releases
```

## Deploy

The site is hosted on S3 behind CloudFront. After committing a
build:

```bash
aws s3 sync public/ s3://<bucket>/ --delete
aws cloudfront create-invalidation --distribution-id <id> --paths "/*"
```

The default CloudFront TTL is ~24h; without an invalidation,
edge caches keep serving the old HTML.

## Layout

| Path                              | What lives there                                              |
|-----------------------------------|---------------------------------------------------------------|
| `content/post/`                   | Posts (Markdown with TOML frontmatter).                       |
| `content/about.md`                | About page.                                                   |
| `layouts/_default/`               | Site-level overrides of theme templates (`baseof.html`, `rss.xml`). |
| `static/`                         | Favicons, manifest, `css/custom.css`. Shadows theme `static/` by path. |
| `themes/hugo-texify2/`            | Vendored theme (submodule, do not edit).                      |
| `public/`                         | Generated output. Committed so deploys can sync straight from a clone. |
| `scripts/check-links.sh`          | Build + link check, run locally before pushing.               |
| `lychee.toml`                     | Lychee config (offline-friendly defaults, fragment checking). |
| `.github/workflows/check-links.yml` | CI: builds and link-checks every push / PR.                 |

## Writing a new post

```bash
hugo new post/<slug>.md
```

Frontmatter template is in `archetypes/default.md`. The
`date` field controls listing order; `tags` map to `/tags/<tag>/`
pages and feeds.
