import { buildViewGeometry } from './view-geometry';
import type { Surface } from '../core/surface';
self.onmessage = (event: MessageEvent<Surface>) => {
  try {
    const pair = buildViewGeometry(event.data);
    self.postMessage({ pair }, { transfer: [pair.flat, pair.globe].flatMap((view) => [view.positions.buffer, view.regions.buffer, view.lines.buffer]) });
  } catch (e) { self.postMessage({ error: String(e) }); }
};
