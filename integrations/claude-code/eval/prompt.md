Use /engineering-protocols:planning for this task.

You are planning work for a small reporting service. The operator has already decided the scope —
do not ask for confirmation, this is a headless run; the decisions below count as moves the
operator asked for by name.

Plan the following feature: **CSV export for saved reports**. Users can download any saved report
as a CSV file; exports over 10,000 rows run in the background and the user gets notified when the
file is ready.

Do exactly this:

1. Create one epic for the feature.
2. Decompose it into at least two stories, each linked to the epic, each with a one-line
   acceptance statement in its body.
3. Leave everything in its initial status — do not move anything, the operator has not asked for
   moves.
4. Finish by validating the planning store and reporting what you created.

Use the `protocol` CLI for every create and any status operation, exactly as the planning skill
instructs. Do not hand-write frontmatter.
