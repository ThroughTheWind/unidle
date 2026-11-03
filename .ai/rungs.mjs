import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';

const pinnedPackageSpec = '@rungs/cli@0.5.0';
const pinnedVersion = pinnedPackageSpec.slice('@rungs/cli@'.length);
const requested = process.argv.slice(2);

// The launcher cannot discover an upgrade while running its current pin. An
// upgrade therefore names the next exact version explicitly; the newer CLI
// rewrites this managed file to make that version the normal pin. Tags, ranges,
// URLs and paths are refused.
const exactVersion = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?(?:\+[0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?$/;
let selectedVersion = pinnedVersion;
if (requested[0] === 'upgrade' && requested[1] === '--to') {
  if (!exactVersion.test(requested[2] ?? '')) {
    console.error('rungs launcher: upgrade --to requires an exact version such as 1.2.3 or 1.2.3-beta.1');
    process.exit(1);
  }
  selectedVersion = requested[2];
  requested.splice(1, 2);
} else if (requested.includes('--to')) {
  console.error('rungs launcher: --to is only valid immediately after upgrade');
  process.exit(1);
}

const packageSpec = selectedVersion === pinnedVersion ? pinnedPackageSpec : `@rungs/cli@${selectedVersion}`;
const npmArgs = ['exec', '--yes', `--package=${packageSpec}`, '--', 'rungs', ...requested];

// npm is a JavaScript program. Calling its entry point through this Node process
// avoids a shell on every platform, so an argument cannot become shell syntax.
// npm sets npm_execpath when it launched the parent; standard Node installers
// also place npm beside node. POSIX can safely fall back to its executable.
const adjacentNpm = join(dirname(process.execPath), 'node_modules', 'npm', 'bin', 'npm-cli.js');
const inheritedNpm = process.env.npm_execpath;
const npmCli = inheritedNpm && existsSync(inheritedNpm) ? inheritedNpm : existsSync(adjacentNpm) ? adjacentNpm : undefined;

if (!npmCli && process.platform === 'win32') {
  console.error('rungs launcher: could not locate npm-cli.js beside Node; install npm or invoke this command from npm');
  process.exitCode = 1;
} else {
  const child = npmCli
    ? spawnSync(process.execPath, [npmCli, ...npmArgs], { stdio: 'inherit', windowsHide: true })
    : spawnSync('npm', npmArgs, { stdio: 'inherit' });

  if (child.error) {
    console.error(`rungs launcher: ${child.error.message}`);
    process.exitCode = 1;
  } else {
    process.exitCode = child.status ?? 1;
  }
}
