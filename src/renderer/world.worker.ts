import { generateWorld } from '../core/world';

self.onmessage = (event: MessageEvent<unknown>) => {
  try {
    const world = generateWorld(event.data, (message) => self.postMessage({ type: 'progress', message }));
    self.postMessage({ type: 'complete', world });
  } catch (error) {
    self.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) });
  }
};
