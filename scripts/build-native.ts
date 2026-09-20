import { execFileSync } from 'node:child_process';
import { copyFile, mkdir, rename } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';

execFileSync('cargo', ['build', '--release', '--locked', '--manifest-path', 'native/Cargo.toml'], { stdio: 'inherit' });
await mkdir('dist/native', { recursive: true });
const binary = process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core';
// Replace the directory entry, not an executable inode that a running session uses.
const staged = `dist/native/.${binary}-${randomUUID()}`;
await copyFile(`native/target/release/${binary}`, staged);
await rename(staged, `dist/native/${binary}`);
