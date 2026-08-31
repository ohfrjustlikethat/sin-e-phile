---
name: decide
description: List every open decision with options, honest costs, and a recommendation. Use when the user types /decide or asks what needs deciding.
---

# /decide

Every decision currently waiting on the author, in a form they can answer in two
words.

## Sources

- `PROJECT_STATE.json` → `decisions_pending` and any blocker with `needs_user: true`
- `docs/DECISIONS_PENDING.md` for the long form
- Anything raised this session and not yet resolved

## For each decision, exactly this

```
P<n> — <the decision in one sentence>
  Why now:    <what it blocks, and what it costs to defer>
  Option A:   <what happens> — <honest cost>
  Option B:   <what happens> — <honest cost>
  Recommend:  <one of them> because <the reason, in one line>
  Default:    <what you will do if the answer is "your call">
```

**The default line is not optional.** It means the author can answer with two words
and you keep moving. A decision presented without a default is a decision that blocks
the project on a reply.

## Rules

- **Never dump the raw problem.** Doing the analysis and handing over the conclusion
  is the job; handing over the confusion is not.
- Costs must be honest and specific, including for your own recommendation. "Slightly
  more work" is not a cost; "an extra `cargo sqlx prepare` after every schema change
  for 24 more phases" is.
- If two options are genuinely close, say so — do not manufacture a preference.
- If a decision has a reasonable default and nothing is blocked, **take the default,
  record it, and tell the author you did.** Do not queue it here.
- Order by when the answer is actually needed, soonest first. Say what happens if it
  is not answered by then.
- Include decisions the author has already made but that are not yet recorded
  anywhere durable — those are the ones that get lost.
