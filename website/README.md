# The public documentation website

A [Docusaurus](https://docusaurus.io/) site: the public, written introduction to this project, for a
reader who has never seen the repository.

## What belongs here, and what does not

This directory and the repository's `docs/` directory are two different artifacts with two different
audiences, and they must not converge.

| | `docs/` | `website/` |
|---|---|---|
| Audience | contributors and agents working *on* this repository | anyone, arriving cold |
| Holds | design documents, reviews, plans, reconciliation gates — including proposals nobody has accepted | pages written for this site |
| Relationship | the internal engineering record | *draws on* that record, in its own words |

Nothing in the build reads outside `website/`. There are no symlinks into `docs/`, no plugin paths
pointing at it, and no imports across the boundary. Source documents are read by a person and written
from; they are not ingested.

Code and artifacts quoted on the site are copied verbatim from `examples/`, `generated/`,
`principles/` and the crates, and every page names its sources at the bottom.

## Running it

```bash
npm install
npm start          # http://localhost:3000/engineering-protocols/
npm run build      # static build into website/build/
npm run serve      # serve the build, also on port 3000
```

Broken links fail the build (`onBrokenLinks: 'throw'`), and that is deliberate — a site whose subject
is checkable claims does not ship dead links.

## Deploying

Build here and publish `website/build/` to the `gh-pages` branch, or upload it as a GitHub Actions
Pages artifact.

**Do not configure GitHub Pages to publish from the `/docs` folder on the default branch.** That
option exists, it is one click away, and here it would publish the internal engineering record —
unaccepted design proposals and defect catalogues included — at the project's public URL. The same
warning is repeated in `docusaurus.config.ts`, where the next person to change the deployment
settings will be looking.
