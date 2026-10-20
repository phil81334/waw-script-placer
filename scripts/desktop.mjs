import { spawn } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { delimiter, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const [command, ...args] = process.argv.slice(2);
const env = { ...process.env };

// Also works in terminals opened before Rust was installed.
const pathKey =
  Object.keys(env).find((key) => {
    return key.toLowerCase() === 'path';
  }) || 'PATH';
env[pathKey] = [
  resolve(env.CARGO_HOME || resolve(homedir(), '.cargo'), 'bin'),
  dirname(process.execPath),
  env[pathKey] || '',
].join(delimiter);

// Development writes to a local test folder unless WAW_ROOT is explicitly supplied.
if (command === 'dev') {
  env.WAW_ROOT ||= resolve(root, '.local', 'test-root');
  mkdirSync(env.WAW_ROOT, { recursive: true });
  console.log(`Development output: ${env.WAW_ROOT}`);
}

const child = spawn(
  process.execPath,
  [resolve(root, 'node_modules/@tauri-apps/cli/tauri.js'), command, ...args],
  { cwd: root, env, stdio: 'inherit' },
);
child.on('error', (error) => {
  console.error(
    `${error.message}\nInstall the Windows prerequisites linked in README.md.`,
  );
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;

  // Put the finished portable app in one easy-to-find folder.
  if (code === 0 && command === 'build') {
    try {
      mkdirSync(resolve(root, 'release'), { recursive: true });
      copyFileSync(
        resolve(root, 'src-tauri/target/release/waw-script-placer.exe'),
        resolve(root, 'release/waw-script-placer.exe'),
      );
      console.log('Ready: release/waw-script-placer.exe');
    } catch (error) {
      console.error(
        `Could not copy the executable to release: ${error.message}`,
      );
      process.exitCode = 1;
    }
  }
});
