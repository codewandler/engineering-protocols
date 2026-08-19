# Artifact definitions

Reference definitions for the engineering artifact graph. The authoritative model is Rust
(`aep-domain::artifact`); these documents carry the parts that are data.

| Directory | Contents |
|---|---|
| `kinds/` | per-kind expectations, such as the sections a design is expected to contain |
| `relations/` | relation vocabulary and which kinds may be related how |
| `lifecycles/` | legal statuses and status transitions per kind — an ADR goes `proposed → accepted → superseded`, a design goes `draft → in_review → approved → implemented` |
| `templates/` | starting points for humans; replaceable without leaving the protocol |
