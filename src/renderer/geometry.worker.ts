import { buildViewGeometry } from './view-geometry';
import type { Surface } from '../core/surface';
import type { Tectonics } from '../core/tectonics';
import type { Water } from '../core/water';
self.onmessage = (event: MessageEvent<{ surface: Surface; tectonics: Tectonics; elevation: Float64Array; water: Water }>) => {
  try {
    const pair = buildViewGeometry(event.data.surface, event.data.tectonics, event.data.elevation, event.data.water);
    self.postMessage({ pair }, { transfer: [pair.flat, pair.globe].flatMap((view) => Object.values(view).map((array) => array.buffer)) });
  } catch (e) { self.postMessage({ error: String(e) }); }
};
