import { spawn } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
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
      const { version } = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'));
      const versionDir = resolve(root, 'release', `v${version}`);
      const executable = resolve(versionDir, 'waw-script-placer.exe');
      const builtExecutable = resolve(root, 'src-tauri/target/release/waw-script-placer.exe');
      if (existsSync(resolve(versionDir, 'release-info.md')) && existsSync(executable)) {
        console.log(`Release v${version} is already archived. Your new build is at ${builtExecutable}`);
        return;
      }

      mkdirSync(versionDir, { recursive: true });
      copyFileSync(builtExecutable, executable);
      console.log(`Ready: ${executable}`);
    } catch (error) {
      console.error(
        `Could not copy the executable to release: ${error.message}`,
      );
      process.exitCode = 1;
    }
  }
});
