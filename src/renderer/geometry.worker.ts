import { buildRenderViewGeometry } from './view-geometry';
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
  effectiveReliefMeters: Float32Array;
  valleyFactor: Float32Array;
};

type LodRequestMessage = {
  type: 'lod';
  requestId: number;
  focus: readonly [number, number, number];
  distance: number;
  tiltRad: number;
  forward: readonly [number, number, number];
  seedOffset: readonly [number, number, number];
  detailEnabled: boolean;
};

type WorkerMessage = BuildMessage | LodFieldsMessage | LodRequestMessage;

let lodCache: LodWorkerCache | null = null;
let lodFields: Omit<LodRequestParams, 'focus' | 'distance' | 'tiltRad' | 'forward' | 'seedOffset' | 'detailEnabled'> | null = null;

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
        effectiveReliefMeters: msg.effectiveReliefMeters,
        valleyFactor: msg.valleyFactor,
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
        tiltRad: msg.tiltRad,
        forward: msg.forward,
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

    const pair = buildRenderViewGeometry(msg.surface, msg.tectonics, msg.elevation, msg.water);
    // Keep compact 6-corner topology and elevation in worker for fast camera-local LOD builds.
    lodCache = pair.compactRing
      ? buildLodCache(
          msg.surface,
          msg.elevation,
          new Uint32Array(0),
          pair.corners.offsets,
          pair.corners.regions,
          {
            ringOffsets: pair.compactRing.ringOffsets.slice(),
            ringCorners: pair.compactRing.ringCorners.slice(),
            ringDirections: pair.compactRing.ringDirections.slice(),
          },
        )
      : null;
    lodFields = null;
    const transferSet = new Set<ArrayBuffer>();
    for (const view of [pair.flat, pair.globe]) {
      for (const value of Object.values(view)) {
        if (ArrayBuffer.isView(value)) transferSet.add(value.buffer as ArrayBuffer);
      }
    }
    if (pair.compactRing) {
      transferSet.add(pair.compactRing.ringOffsets.buffer as ArrayBuffer);
      transferSet.add(pair.compactRing.ringCorners.buffer as ArrayBuffer);
      transferSet.add(pair.compactRing.ringDirections.buffer as ArrayBuffer);
    }
    self.postMessage({ type: 'build', pair }, { transfer: [...transferSet] });
  } catch (e) {
    self.postMessage({ error: String(e) });
  }
};
