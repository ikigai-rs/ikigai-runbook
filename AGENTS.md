# Working in this repository

`ikigai-runbook` is an ikigai module: guided, runnable demos served as `urn:runbook:*` resources,
rendered as an htmx tab strip and step panel in a browser or as text in a terminal, and linked by
both the CLI and the in-browser kernel. These rules are for every agent and every person working
here. `CLAUDE.md` is a link to this file.

- **Read [ACCESSIBILITY.md](ACCESSIBILITY.md) before any change to UI or rendered output**: the
  HTML a page renders (the tab strip, the steps, the output region), its text face and its JSON
  face. It lists what has been tested, what has not, and the known barriers.
- Security reports go through the organization's security policy (GitHub private vulnerability
  reporting), never a public issue.
- Every change lands through a pull request with green CI (`.github/workflows/ci.yml`: fmt,
  clippy with warnings denied, tests, docs, and the wasm build of the library).
- Never invent testing, tool results or conformance evidence, and never weaken a test to make a
  change pass.
- Use American spelling in everything you write: code comments, docs, commit messages and pull
  requests.
