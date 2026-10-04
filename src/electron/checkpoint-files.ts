import { randomUUID } from 'node:crypto';
import { open, rename, unlink } from 'node:fs/promises';

/** Reject oversize files and growth during reading without unbounded allocation. */
export async function readCheckpoint(filePath: string, limit: number, label: string): Promise<string> {
  const file = await open(filePath, 'r');
  try {
    const stat = await file.stat();
    if (!stat.isFile()) throw new Error(`${label} must be a regular file.`);
    const tooLarge = (): Error => new Error(`${label} exceeds the ${limit / 2 ** 20} MiB limit.`);
    if (stat.size > limit) throw tooLarge();
    const chunks: Buffer[] = [];
    let count = 0;
    for (;;) {
      const chunk = Buffer.allocUnsafe(Math.min(65536, limit + 1 - count));
      const { bytesRead } = await file.read(chunk, 0, chunk.length, count);
      if (bytesRead === 0) break;
      count += bytesRead;
      if (count > limit) throw tooLarge();
      chunks.push(chunk.subarray(0, bytesRead));
    }
    // Do not silently replace invalid UTF-8 in a supposedly exact state file.
    return new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks, count));
  } finally { await file.close(); }
}

/** Publish only a fully written/synced file. An ordinary write error retains the target. */
export async function writeCheckpoint(filePath: string, contents: string): Promise<void> {
  const temporary = `${filePath}.tmp-${randomUUID()}`;
  let created = false;
  try {
    const file = await open(temporary, 'wx');
    created = true;
    try { await file.writeFile(contents, 'utf8'); await file.sync(); }
    finally { await file.close(); }
    await rename(temporary, filePath);
  } catch (error) {
    if (created) await unlink(temporary).catch(() => {});
    throw error;
  }
}
