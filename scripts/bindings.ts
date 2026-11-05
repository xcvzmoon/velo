import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import process from 'node:process';

const output = execFileSync(
  'cargo',
  ['run', '--locked', '-q', '-p', 'velo-contracts', '--example', 'export-bindings'],
  {
    encoding: 'utf8',
  },
);

const formatted = execFileSync(
  'pnpm',
  ['exec', 'oxfmt', '--stdin-filepath', 'app/types/contracts.ts'],
  {
    input: output,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  },
);

const target = new URL('../app/types/contracts.ts', import.meta.url);

if (process.argv.includes('--check')) {
  if (readFileSync(target, 'utf8') !== formatted) {
    throw new Error('BINDINGS_OUT_OF_DATE: run pnpm bindings:generate');
  }
} else {
  writeFileSync(target, formatted);
}
