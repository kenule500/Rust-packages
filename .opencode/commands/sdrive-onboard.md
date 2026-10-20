---
description: "Onboard an existing project into SpecDrive"
---

# /sdrive:onboard

Onboard an existing project into SpecDrive

Run the SpecDrive command `/sdrive:onboard`.

1. Read `.sdrive/commands/manifest.json` and find the entry for this command.
2. Read the agent file it names in `agentFile` (`.sdrive/agents/00-onboarding.md`).
3. Follow that agent's instructions exactly.
4. Run the validators the manifest lists for this command.
5. Stop at every human approval gate the agent defines.

Arguments: $ARGUMENTS
