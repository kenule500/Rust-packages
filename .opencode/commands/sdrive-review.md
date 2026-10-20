---
description: "Final audit — pass/fail verdict, no merge"
argument-hint: "<feature> | --all"
---

# /sdrive:review

Final audit — pass/fail verdict, no merge

Run the SpecDrive command `/sdrive:review`.

1. Read `.sdrive/commands/manifest.json` and find the entry for this command.
2. Read the agent file it names in `agentFile` (`.sdrive/agents/09-review-complete.md`).
3. Follow that agent's instructions exactly.
4. Run the validators the manifest lists for this command.
5. Stop at every human approval gate the agent defines.

Arguments: $ARGUMENTS
