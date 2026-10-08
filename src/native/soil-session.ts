import { parseRecipe } from '../core/recipe';
import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import { MOISTURE_MAX_SECONDS } from '../shared/seasonal-moisture';
import { MAX_SEASONAL_CHECKPOINT_BYTES } from '../shared/seasonal-checkpoint';
import { SOIL_MOISTURE_CONTRACT } from '../shared/soil-moisture';
import type { SoilMoistureFrame } from '../shared/soil-moisture';
import { NativeSession, decodeWorld } from './client';
import { decodeSoilMoisture } from './soil-moisture';
import { matches } from './water-validation';

function metadata(contents: string): { recipe: Recipe; seconds: number } {
  if (typeof contents !== 'string' || Buffer.byteLength(contents) > MAX_SEASONAL_CHECKPOINT_BYTES) {
    throw new Error('Soil-water checkpoint exceeds the 64 MiB limit or is not JSON text.');
  }
  const saved = JSON.parse(contents) as Record<string, unknown> | null;
  if (!saved || saved.schemaVersion !== SOIL_MOISTURE_CONTRACT.schema
    || saved.modelVersion !== SOIL_MOISTURE_CONTRACT.modelVersion
    || !Number.isSafeInteger(saved.elapsedSeconds) || (saved.elapsedSeconds as number) < 0
    || (saved.elapsedSeconds as number) > MOISTURE_MAX_SECONDS) {
    throw new Error('Unsupported soil-water checkpoint version or clock.');
  }
  return { recipe: parseRecipe(saved.recipe), seconds: saved.elapsedSeconds as number };
}

/** Independent typed transport, not yet an Electron mode. A restored candidate
 * owns a fresh process and never replaces another session implicitly. */
export class SoilMoistureSession {
  private constructor(private readonly native: NativeSession, private readonly initialWorld: World,
    private current: SoilMoistureFrame) {}

  static async initialize(executable: string, recipe: Recipe): Promise<SoilMoistureSession> {
    const resolved = parseRecipe(recipe), native = new NativeSession(executable);
    try {
      const world = decodeWorld(await native.request({ command: 'generate', recipe: resolved }));
      const frame = decodeSoilMoisture(await native.request({ command: 'initializeSoilMoisture' }), world, 1, 0, 0);
      return new SoilMoistureSession(native, world, frame);
    } catch (error) { native.close(); throw error; }
  }

  static async restore(executable: string, contents: string): Promise<SoilMoistureSession> {
    const { recipe, seconds } = metadata(contents), native = new NativeSession(executable);
    try {
      const world = decodeWorld(await native.request({ command: 'generate', recipe }));
      // JS only preflights metadata. Rust receives the original decimal tokens
      // and signed zeros, with no reserialization of checkpointed stock pairs.
      const frame = decodeSoilMoisture(await native.request({ command: 'restoreSoilMoisture', checkpointJson: contents }),
        world, 1, seconds, 0);
      return new SoilMoistureSession(native, world, frame);
    } catch (error) { native.close(); throw error; }
  }

  // Consumers cannot mutate the snapshot used to validate later interval frames.
  get world(): World { return structuredClone(this.initialWorld); }
  get frame(): SoilMoistureFrame { return structuredClone(this.current); }

  async advance(seconds: number): Promise<SoilMoistureFrame> {
    if (!Number.isSafeInteger(seconds) || seconds < 0 || seconds > 86400
      || this.current.elapsedSeconds + seconds > MOISTURE_MAX_SECONDS) throw new Error('Invalid soil-water interval.');
    const previous = this.current;
    const packet = await this.native.request({ command: 'seasonalMoisture', seconds });
    try {
      const frame = decodeSoilMoisture(packet, this.initialWorld, previous.epoch,
        previous.elapsedSeconds + seconds, seconds, previous);
      this.current = frame;
      return structuredClone(frame);
    } catch (error) { this.native.close(); throw error; }
  }

  async checkpoint(): Promise<string> {
    const { header: h, bytes } = await this.native.request({ command: 'exportMoisture' });
    try {
      const contract = SOIL_MOISTURE_CONTRACT;
      if (h.kind !== contract.checkpointKind || h.protocol !== contract.protocol
        || h.schemaVersion !== contract.schema || h.modelVersion !== contract.modelVersion
        || h.elapsedSeconds !== this.current.elapsedSeconds || h.byteLength !== bytes.length
        || !bytes.length || bytes.length >= MAX_SEASONAL_CHECKPOINT_BYTES) throw new Error('Invalid native soil-water checkpoint.');
      const contents = bytes.toString('utf8'), saved = metadata(contents);
      if (saved.seconds !== this.current.elapsedSeconds || !matches(saved.recipe, this.initialWorld.recipe)) {
        throw new Error('Native soil-water checkpoint does not match the selected world.');
      }
      return `${contents}\n`;
    } catch (error) { this.native.close(); throw error; }
  }

  close(): void { this.native.close(); }
}
