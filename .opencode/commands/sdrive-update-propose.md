---
description: "Extend an existing feature specification with new requirements, stories, and acceptance criteria"
argument-hint: "<feature>"
---

# /sdrive:update-propose

Extend an existing feature specification with new requirements, stories, and acceptance criteria

Run the SpecDrive command `/sdrive:update-propose`.

1. Read `.sdrive/commands/manifest.json` and find the entry for this command.
2. Read the agent file it names in `agentFile` (`.sdrive/agents/13-update-propose.md`).
3. Follow that agent's instructions exactly.
4. Run the validators the manifest lists for this command.
5. Stop at every human approval gate the agent defines.

Arguments: $ARGUMENTS
