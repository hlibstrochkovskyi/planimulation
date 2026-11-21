import { buildViewGeometry } from './view-geometry';
import { buildLodCache, buildLodPatch } from './refinement';
import type { LodWorkerCache, LodRequestParams } from './refinement';
import type { Surface } from '../core/surface';
import type { Tectonics } from '../core/tectonics';
import type { Water } from '../core/water';

type BuildMessage = {
  type?: 'build';
  surface: Surface;
  tectonics: Tectonics;
  elevation: Float64Array;
  water: Water;
};

type LodFieldsMessage = {
  type: 'lod-fields';
  waterLevelMeters: number;
  wetMask: Uint8Array;
  phi: Float32Array;
  strictMask: Float32Array;
  localWaterLevel: Float32Array;
  localReliefMeters: Float32Array;
};

type LodRequestMessage = {
  type: 'lod';
  requestId: number;
  focus: readonly [number, number, number];
  distance: number;
  seedOffset: readonly [number, number, number];
  detailEnabled: boolean;
};

type WorkerMessage = BuildMessage | LodFieldsMessage | LodRequestMessage;

let lodCache: LodWorkerCache | null = null;
let lodFields: Omit<LodRequestParams, 'focus' | 'distance' | 'seedOffset' | 'detailEnabled'> | null = null;

self.onmessage = (event: MessageEvent<WorkerMessage>) => {
  try {
    const msg = event.data;
    if (msg.type === 'lod-fields') {
      lodFields = {
        waterLevelMeters: msg.waterLevelMeters,
        wetMask: msg.wetMask,
        phi: msg.phi,
        strictMask: msg.strictMask,
        localWaterLevel: msg.localWaterLevel,
        localReliefMeters: msg.localReliefMeters,
      };
      return;
    }
    if (msg.type === 'lod') {
      if (!lodCache || !lodFields) {
        self.postMessage({ type: 'lod', requestId: msg.requestId, patch: null });
        return;
      }
      const patch = buildLodPatch(lodCache, {
        ...lodFields,
        focus: msg.focus,
        distance: msg.distance,
        seedOffset: msg.seedOffset,
        detailEnabled: msg.detailEnabled,
      });
      const transfer = [
        patch.positions.buffer,
        patch.regions.buffer,
        patch.cornerPairs.buffer,
        patch.weights.buffer,
        patch.radialOffsets.buffer,
        patch.replacedRegions.buffer,
      ];
      self.postMessage({ type: 'lod', requestId: msg.requestId, patch }, { transfer });
      return;
    }

    const pair = buildViewGeometry(msg.surface, msg.tectonics, msg.elevation, msg.water);
    // Keep lightweight topology/elevation references in worker for fast camera-local LOD builds.
    lodCache = pair.corners.boundaryCorners
      ? buildLodCache(msg.surface, msg.elevation, pair.corners.boundaryCorners, pair.corners.offsets, pair.corners.regions)
      : null;
    lodFields = null;
    const transfer = [pair.flat, pair.globe].flatMap((view) => Object.values(view).map((array) => array.buffer));
    self.postMessage({ type: 'build', pair }, { transfer });
  } catch (e) {
    self.postMessage({ error: String(e) });
  }
};
