---
description: "Generate documentation and detect drift"
argument-hint: "<feature> | --all"
---

# /sdrive:docs

Generate documentation and detect drift

Run the SpecDrive command `/sdrive:docs`.

1. Read `.sdrive/commands/manifest.json` and find the entry for this command.
2. Read the agent file it names in `agentFile` (`.sdrive/agents/06-documentation.md`).
3. Follow that agent's instructions exactly.
4. Run the validators the manifest lists for this command.
5. Stop at every human approval gate the agent defines.

Arguments: $ARGUMENTS
