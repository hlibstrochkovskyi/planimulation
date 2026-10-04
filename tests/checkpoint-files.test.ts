import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { mkdtemp, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { readCheckpoint, writeCheckpoint } from '../src/electron/checkpoint-files';

test('checkpoint files have bounded strict reads and publish complete writes without losing existing targets', async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'planimulation-checkpoint-files-'));
  const file = path.join(directory, 'checkpoint.json');
  const original = '{"value":-0.0,"exact":1.2345678901234567}\n';
  await writeCheckpoint(file, original);
  assert.equal(await readCheckpoint(file, Buffer.byteLength(original), 'Checkpoint'), original);
  await assert.rejects(readCheckpoint(file, Buffer.byteLength(original) - 1, 'Checkpoint'), /exceeds/);
  await assert.rejects(readCheckpoint(directory, 1024, 'Checkpoint'), /regular file/);
  const invalid = path.join(directory, 'invalid.json'); await writeFile(invalid, Buffer.from([0xff]));
  await assert.rejects(readCheckpoint(invalid, 1024, 'Checkpoint'));
  // Exercise several reads rather than relying on one full-sized read result.
  const longer = JSON.stringify({ text: 'a'.repeat(150000) });
  await writeCheckpoint(file, longer);
  assert.equal(await readCheckpoint(file, Buffer.byteLength(longer), 'Checkpoint'), longer);
  const destinationDirectory = path.join(directory, 'destination'); await mkdir(destinationDirectory);
  const retained = path.join(destinationDirectory, 'retained.txt'); await writeFile(retained, original);
  await assert.rejects(writeCheckpoint(destinationDirectory, longer));
  assert.equal(await readFile(retained, 'utf8'), original);
  assert.ok((await readdir(directory)).every((name) => !name.includes('.tmp-')));
  await assert.rejects(writeCheckpoint(path.join(directory, 'missing', 'checkpoint.json'), longer));
});
