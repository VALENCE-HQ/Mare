#!/usr/bin/env node
// Cuts a release from your own machine:
//   1. checks the tag matches package.json
//   2. installs dependencies and sanity-checks the CLI
//   3. runs `npm pack`
//   4. pushes, then creates a GitHub Release with the .tgz attached and auto-generated notes
//      (versions with a "-" in them, like 0.4.0-beta.1, are marked as prereleases)
//
// Usage:
//   npm run release                 release the version already in package.json
//   npm run release -- patch        bump first (patch | minor | major | prerelease | 1.2.3)
//   npm run release -- patch --dry-run    do all the checks, push and publish nothing
//   --npm      also publish to npm
//   --draft    create the GitHub release as a draft
//
// Needs: git, the GitHub CLI (`gh`, logged in), Node 18+.

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
process.chdir(root);

const isWin = process.platform === 'win32';
const argv = process.argv.slice(2);
const has = (flag) => argv.includes(flag);
const bump = argv.find((a) => !a.startsWith('--'));
const dryRun = has('--dry-run');
const withNpm = has('--npm');
const draft = has('--draft');

// ---- helpers ---------------------------------------------------------------

function die(msg) {
  console.error(`\n✖ ${msg}\n`);
  process.exit(1);
}

function step(n, msg) {
  console.log(`\n[${n}] ${msg}`);
}

function exec(cmd, args, { inherit = false } = {}) {
  const r = spawnSync(cmd, args, {
    encoding: 'utf8',
    shell: isWin,
    stdio: inherit ? 'inherit' : ['ignore', 'pipe', 'pipe'],
  });
  return { ok: r.status === 0 && !r.error, out: (r.stdout || '').trim(), err: (r.stderr || '').trim() };
}

/** Run, return stdout, die on failure. */
function out(cmd, args) {
  const r = exec(cmd, args);
  if (!r.ok) die(`"${cmd} ${args.join(' ')}" failed:\n${r.err || r.out}`);
  return r.out;
}

/** Run with visible output, die on failure. */
function show(cmd, args) {
  const r = exec(cmd, args, { inherit: true });
  if (!r.ok) die(`"${cmd} ${args.join(' ')}" failed.`);
}

const readVersion = () => JSON.parse(fs.readFileSync('package.json', 'utf8')).version;

let tarball = null;
function cleanup() {
  if (tarball && fs.existsSync(tarball)) fs.rmSync(tarball);
}
process.on('exit', cleanup);

// ---- preflight ---------------------------------------------------------------

step(0, 'Preflight');
if (!exec('git', ['--version']).ok) die('git was not found.');
if (!exec('gh', ['--version']).ok) die('The GitHub CLI (gh) was not found. Install it from https://cli.github.com');
if (!exec('gh', ['auth', 'status']).ok) die('gh is not logged in. Run: gh auth login');
if (!exec('git', ['rev-parse', '--is-inside-work-tree']).ok) die('This is not a git repository.');
if (!exec('git', ['remote', 'get-url', 'origin']).ok) die('No "origin" remote. Add one with: git remote add origin <url>');

const branch = out('git', ['rev-parse', '--abbrev-ref', 'HEAD']);
if (branch === 'HEAD') die('You are on a detached HEAD. Check out a branch first.');

const dirty = out('git', ['status', '--porcelain']);
if (dirty) {
  if (dryRun) console.log('! Uncommitted changes (ignored for a dry run):\n' + dirty);
  else die('You have uncommitted changes. Commit or stash them first:\n' + dirty);
}
console.log(`✓ on branch "${branch}", gh is logged in${dryRun ? ' (dry run: nothing will be pushed)' : ''}`);

// ---- version + tag -----------------------------------------------------------

step(1, 'Version and tag');
if (bump && !dryRun) {
  // Bumps package.json, commits "Release vX.Y.Z" and creates the tag
  show('npm', ['version', bump, '-m', 'Release v%s']);
} else if (bump && dryRun) {
  console.log(`(dry run) would run: npm version ${bump}`);
}

const version = readVersion();
const tag = `v${version}`;
const prerelease = version.includes('-');

const tagExists = exec('git', ['rev-parse', '-q', '--verify', `refs/tags/${tag}`]).ok;
if (tagExists) {
  const tagCommit = out('git', ['rev-list', '-n', '1', tag]);
  const head = out('git', ['rev-parse', 'HEAD']);
  if (tagCommit !== head) {
    die(`Tag ${tag} already exists but points at a different commit than HEAD.\nBump the version (npm run release -- patch) instead of reusing it.`);
  }
  console.log(`✓ tag ${tag} exists and matches HEAD`);
} else if (dryRun) {
  console.log(`(dry run) would create tag ${tag}`);
} else {
  out('git', ['tag', '-a', tag, '-m', tag]);
  console.log(`✓ created tag ${tag}`);
}
console.log(`✓ tag ${tag} matches package.json version ${version}${prerelease ? ' (prerelease)' : ''}`);

if (exec('gh', ['release', 'view', tag]).ok) die(`A GitHub release for ${tag} already exists.`);

// ---- checks + pack ------------------------------------------------------------

step(2, 'Install dependencies and sanity-check the CLI');
show('npm', ['install']);
show('node', ['--check', 'index.js']);
const cliVersion = out('node', ['index.js', '--version']);
if (cliVersion !== version) die(`CLI reports version ${cliVersion} but package.json says ${version}.`);
console.log(`✓ CLI runs and reports ${cliVersion}`);

step(3, 'npm pack');
const packed = out('npm', ['pack', '--silent']).split('\n').pop().trim();
tarball = path.resolve(packed);
if (!fs.existsSync(tarball)) die(`npm pack did not produce ${packed}.`);
console.log(`✓ built ${packed}`);

if (dryRun) {
  console.log(`\nDry run complete. Would push ${branch} + ${tag}, create the GitHub release${withNpm ? ' and publish to npm' : ''}.`);
  process.exit(0);
}

// ---- push + release -----------------------------------------------------------

step(4, 'Push and create the GitHub release');
show('git', ['push', 'origin', 'HEAD']);
show('git', ['push', 'origin', tag]);

const releaseArgs = ['release', 'create', tag, tarball, '--title', tag, '--generate-notes', '--verify-tag'];
if (prerelease) releaseArgs.push('--prerelease');
if (draft) releaseArgs.push('--draft');
show('gh', releaseArgs);

if (withNpm) {
  step(5, 'Publish to npm');
  if (!exec('npm', ['whoami']).ok) die('Not logged in to npm. Run: npm login');
  show('npm', ['publish', '--access', 'public', '--tag', prerelease ? 'next' : 'latest']);
}

const url = exec('gh', ['release', 'view', tag, '--json', 'url', '-q', '.url']).out;
console.log(`\n✓ Released ${tag}${url ? `\n  ${url}` : ''}\n`);
