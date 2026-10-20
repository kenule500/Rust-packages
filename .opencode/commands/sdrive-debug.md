---
description: "Reproduce, isolate, and fix defects — layout, APIs, runtime, and glitches"
argument-hint: "<feature> | --all"
---

# /sdrive:debug

Reproduce, isolate, and fix defects — layout, APIs, runtime, and glitches

Run the SpecDrive command `/sdrive:debug`.

1. Read `.sdrive/commands/manifest.json` and find the entry for this command.
2. Read the agent file it names in `agentFile` (`.sdrive/agents/15-debug.md`).
3. Follow that agent's instructions exactly.
4. Run the validators the manifest lists for this command.
5. Stop at every human approval gate the agent defines.

Arguments: $ARGUMENTS
