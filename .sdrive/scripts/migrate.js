#!/usr/bin/env node

console.error("⚠️  `sdrive migrate` is deprecated. Use the migration adapter commands.");
console.error("");
console.error("Detect:   sdrive migrate:detect");
console.error("Plan:     sdrive migrate:plan <framework>");
console.error("Apply:    sdrive migrate:apply <framework> --yes");
console.error("Verify:   sdrive migrate:verify <migration-id>");
console.error("Rollback: sdrive migrate:rollback <migration-id>");
console.error("");
console.error("Frameworks: openspec, speckit, kiro, agent-os, bmad, generic.");
console.error("The Onboarding Agent runs the same flow after the inventory is approved.");
process.exit(1);
