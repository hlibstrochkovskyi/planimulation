/** FNV-1a over UTF-8 bytes; multiplication and state wrap at 32 bits. */
export function hashString(value: string): number {
  let hash = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(value)) {
    hash = Math.imul(hash ^ byte, 0x01000193) >>> 0;
  }
  return hash;
}

export class Random {
  private state: number;

  constructor(state: number) {
    this.state = state >>> 0;
  }

  nextUint32(): number {
    this.state = (this.state + 0x6d2b79f5) >>> 0;
    let value = this.state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return (value ^ (value >>> 14)) >>> 0;
  }

  next(): number {
    return this.nextUint32() / 4294967296;
  }

  snapshot(): number {
    return this.state;
  }
}

export function stream(seed: string, name: string): Random {
  // JSON encoding keeps stream names unambiguous even when seeds contain separators.
  return new Random(hashString(JSON.stringify([seed, name])));
}
