---
description: "Merge an approved feature to the confirmed target branch and complete its lifecycle"
argument-hint: "<feature> | --all"
---

# /sdrive:archive

Merge an approved feature to the confirmed target branch and complete its lifecycle

Run the SpecDrive command `/sdrive:archive`.

1. Read `.sdrive/commands/manifest.json` and find the entry for this command.
2. Read the agent file it names in `agentFile` (`.sdrive/agents/14-archive.md`).
3. Follow that agent's instructions exactly.
4. Run the validators the manifest lists for this command.
5. Stop at every human approval gate the agent defines.

Arguments: $ARGUMENTS
