#!/usr/bin/env node
'use strict';

// Validates the design-partner documents against their acceptance criteria.
//
// Run from the repository root unattended:
//
//   node scripts/docs-partners.js
//
// Exits 0 when every document is complete and internally consistent, 1 otherwise.

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const DIR = path.join(ROOT, 'docs', 'design-partners');

const PROGRAM = 'program.md';
const TRACKER = 'tracker.md';
const TEMPLATE = 'case-study-template.md';

const COLUMNS = [
  'project',
  'contact role',
  'crate in use',
  'environment',
  'start date',
  'status',
  'evidence link',
];

const STATUSES = ['prospect', 'active', 'case study published', 'declined'];

const PLACEHOLDER = /^<[^>]+>$/;

const failures = [];

function fail(message) {
  failures.push(message);
}

function read(name) {
  const file = path.join(DIR, name);
  if (!fs.existsSync(file)) {
    fail('missing docs/design-partners/' + name);
    return null;
  }
  return fs.readFileSync(file, 'utf8');
}

function flat(text) {
  return text.replace(/\s+/g, ' ');
}

function cells(row) {
  return row
    .split('|')
    .slice(1, -1)
    .map((cell) => cell.trim());
}

function unquote(cell) {
  return cell.replace(/^`|`$/g, '');
}

// Markdown renders the cells of a row by dropping the leading and trailing
// pipes, so this splits on the remaining pipes and ignores any width mismatch:
// the rule cares whether the evidence cell is filled, not how many cells exist.
function rowCells(row) {
  let body = row.trim().replace(/^\s*\|/, '').replace(/\|\s*$/, '');
  body = body.split(/(?<!\\)\|$/)[0];
  return body.split(/(?<!\\)\|/).map((cell) => cell.trim());
}

function isPlaceholder(cell) {
  const value = unquote(cell).trim();
  return value === '' || PLACEHOLDER.test(value);
}

// The program page states eligibility, what a partner receives, what a partner
// commits to, the project's obligations, the production definition, and the
// G5 gate, and it says that missing evidence blocks 1.0.
function checkProgram(text) {
  for (const heading of [
    '## Eligibility',
    '## What a partner receives',
    '## What a partner commits to',
    '## Obligations of the project',
    '## What "in production" means',
  ]) {
    if (!text.includes(heading)) {
      fail(PROGRAM + ' is missing the section ' + heading);
    }
  }

  const body = flat(text);

  if (!/G5/i.test(body) || !/three external projects/i.test(body)) {
    fail(PROGRAM + ' must name the G5 gate (three external projects)');
  }

  if (!/30 (consecutive|days)/i.test(body)) {
    fail(PROGRAM + ' must define in production as live for at least 30 consecutive days');
  }

  if (!/does not happen|blocks the gate|blocks 1\.0/i.test(body)) {
    fail(PROGRAM + ' must state that missing evidence blocks 1.0');
  }
}

// The tracker is an empty template: seven columns, the four status values, and no
// row that names a partner without an evidence link.
function checkTracker(text) {
  const lines = text.split(/\r?\n/);

  const headerIndex = lines.findIndex((line) => /^\|\s*Project\s*\|/.test(line));
  if (headerIndex === -1) {
    fail(TRACKER + ' has no Partners table header');
    return;
  }

  const header = rowCells(lines[headerIndex]).map((cell) => cell.toLowerCase());
  for (const column of COLUMNS) {
    if (!header.includes(column)) {
      fail(TRACKER + ' table is missing the "' + column + '" column');
    }
  }

  for (const status of STATUSES) {
    if (!text.includes(status)) {
      fail(TRACKER + ' does not document the status "' + status + '"');
    }
  }

  const statusIndex = header.indexOf('status');
  const evidenceIndex = header.indexOf('evidence link');

  for (let index = headerIndex + 1; index < lines.length; index += 1) {
    const row = lines[index];

    // The table ends at the first non-table line, which includes a heading.
    if (row.trim() === '' || !row.trim().startsWith('|')) {
      break;
    }

    // Skip the `|---|` alignment row that directly follows the header.
    if (rowCells(row).every((cell) => /^:?-+:?$/.test(cell))) {
      continue;
    }

    const values = rowCells(row);
    const status = unquote(values[statusIndex] || '');
    const evidence = values[evidenceIndex] || '';

    if (!STATUSES.includes(status)) {
      fail(TRACKER + ' row ' + (index + 1) + ' has an unknown status "' + status + '"');
    }

    // Placeholder rows are `<...>` or a backticked `<...>`; both are empty cells.
    if (!isPlaceholder(evidence)) {
      fail(TRACKER + ' row ' + (index + 1) + ' names a partner without a placeholder evidence link');
    }
  }
}

// The case-study template requires consent, redaction, and measured outcomes.
function checkTemplate(text) {
  const body = flat(text);

  if (!/## Consent statement/i.test(text)) {
    fail(TEMPLATE + ' is missing the consent statement');
  }
  if (!/## Redaction checklist/i.test(text)) {
    fail(TEMPLATE + ' is missing the redaction checklist');
  }
  if (!/### Measured outcomes/i.test(text)) {
    fail(TEMPLATE + ' is missing the measured-outcomes section');
  }
  if (!/written approval/i.test(body)) {
    fail(TEMPLATE + ' consent must require written approval');
  }
  if (!/credentials?/i.test(body)) {
    fail(TEMPLATE + ' redaction checklist must cover credentials');
  }
}

// The README links to the program page.
function checkReadme() {
  const readme = fs.readFileSync(path.join(ROOT, 'README.md'), 'utf8');
  if (!readme.includes('docs/design-partners/program.md')) {
    fail('README.md does not link to docs/design-partners/program.md');
  }
}

const program = read(PROGRAM);
if (program) {
  checkProgram(program);
}

const tracker = read(TRACKER);
if (tracker) {
  checkTracker(tracker);
}

const template = read(TEMPLATE);
if (template) {
  checkTemplate(template);
}

checkReadme();

if (failures.length > 0) {
  console.error('docs-partners check failed:');
  for (const failure of failures) {
    console.error('  - ' + failure);
  }
  process.exit(1);
}

console.log('docs-partners check passed');
