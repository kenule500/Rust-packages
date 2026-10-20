---
description: "Write production code from approved tasks, on the branch /sdrive:propose created"
argument-hint: "<feature> | --all"
---

# /sdrive:apply

Write production code from approved tasks, on the branch /sdrive:propose created

Run the SpecDrive command `/sdrive:apply`.

1. Read `.sdrive/commands/manifest.json` and find the entry for this command.
2. Read the agent file it names in `agentFile` (`.sdrive/agents/07-implementation.md`).
3. Follow that agent's instructions exactly.
4. Run the validators the manifest lists for this command.
5. Stop at every human approval gate the agent defines.

Arguments: $ARGUMENTS
