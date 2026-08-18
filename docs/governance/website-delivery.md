---
type: governance
title: Website Delivery
status: draft
version: 0.1.0
---

# Website Delivery

## Product surface

The canonical product website is
`https://bahadirarda.github.io/rebinder/`. Its source lives under `site/` as
dependency-free semantic HTML, CSS, and progressive JavaScript. Core product
content, navigation, install commands, and project links remain available
without client-side rendering.

The website presents both operational transfer directions and the explicit
registered-worktree recovery boundary. It may describe Claude and Codex session
discovery, interactive source selection, context-safe handoffs, native target
session binding, canonical export, capability-aware compatibility reporting,
native command passthrough, session-package validation and inspection, the
interchange format, exact unlocked worktree recovery, and verified native
releases as available. It MUST distinguish this bounded local recovery from
clone, fetch, unlock, overwrite, and uncommitted-state restoration.

## Visual and accessibility contract

The visual system reuses the repository's black, cobalt, cyan, and warm-white
identity. The tracked hero image becomes the social preview image during
deployment rather than being duplicated in the website source.

The source uses landmarks, one page heading, visible focus treatment, a skip
link, labeled controls, live copy feedback, responsive layouts, and a
reduced-motion path. JavaScript progressively adds clipboard actions, install
tabs, the current footer year, and reveal transitions; it is not required to
read or navigate the page.

## Search and agent discovery

The canonical page exposes a descriptive title and summary, crawl directives,
Open Graph metadata, large-image social metadata, and `WebSite` plus
`SoftwareApplication` structured data. `robots.txt` points to the canonical
root sitemap, which contains only the canonical indexable product URL.

The root `llms.txt` gives coding agents a concise product summary and curated
links to raw product, architecture, format, distribution, and source documents.
It explicitly repeats the current two-way transfer and registered-worktree
recovery boundary and is a discovery aid rather than an access-control
mechanism.

## Installer boundary

The website publishes the tracked `site/install.sh` and `site/install.ps1`
without transforming them. The installers resolve an exact native GitHub
Release, download its platform archive and release-owned `SHA256SUMS`, verify
the selected archive, validate embedded release identity, stage activation,
and smoke-test the installed CLI version. They do not require elevated
privileges or edit the user's shell profile.

## Deployment

The `pages` workflow runs after relevant changes reach `main`. It validates the
website, configures GitHub Pages, assembles an isolated `_site` artifact, copies
the tracked hero image as the social card, uploads the Pages artifact, and
deploys through the `github-pages` environment.

Repository content is read-only during both jobs. The deployment receives only
the GitHub Pages and OpenID Connect permissions required by GitHub's custom
Pages workflow. Ordinary pull request CI runs the same structural website
validator before source can reach the default branch.
