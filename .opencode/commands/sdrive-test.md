---
description: "Write tests for acceptance criteria, run them, and report coverage"
argument-hint: "<feature> | --all"
---

# /sdrive:test

Write tests for acceptance criteria, run them, and report coverage

Run the SpecDrive command `/sdrive:test`.

1. Read `.sdrive/commands/manifest.json` and find the entry for this command.
2. Read the agent file it names in `agentFile` (`.sdrive/agents/11-test.md`).
3. Follow that agent's instructions exactly.
4. Run the validators the manifest lists for this command.
5. Stop at every human approval gate the agent defines.

Arguments: $ARGUMENTS
