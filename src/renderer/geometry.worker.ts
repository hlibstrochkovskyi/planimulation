import { buildViewGeometry } from './view-geometry';
import type { Surface } from '../core/surface';
import type { Tectonics } from '../core/tectonics';
self.onmessage = (event: MessageEvent<{ surface: Surface; tectonics: Tectonics }>) => {
  try {
    const pair = buildViewGeometry(event.data.surface, event.data.tectonics);
    self.postMessage({ pair }, { transfer: [pair.flat, pair.globe].flatMap((view) => [view.positions.buffer, view.regions.buffer, view.lines.buffer, view.tectonicLines.buffer, view.tectonicColors.buffer]) });
  } catch (e) { self.postMessage({ error: String(e) }); }
};
