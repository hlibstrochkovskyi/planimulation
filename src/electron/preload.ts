import { contextBridge, ipcRenderer } from 'electron';
import type { DesktopAPI } from '../shared/desktop-api';

const api: DesktopAPI = {
  openRecipe: () => ipcRenderer.invoke('recipe:open'),
  saveRecipe: (recipe) => ipcRenderer.invoke('recipe:save', recipe),
  exportView: (rect) => ipcRenderer.invoke('view:export', rect),
  generate: (recipe) => ipcRenderer.invoke('world:generate', recipe),
  cancelGeneration: () => ipcRenderer.invoke('world:cancel'),
  acceptWorld: (epoch) => ipcRenderer.invoke('world:accept', epoch),
  advance: (epoch, steps) => ipcRenderer.invoke('world:advance', epoch, steps),
  prescribeWater: (epoch, region, mode) => ipcRenderer.invoke('world:prescribeWater', epoch, region, mode),
  openWaterCheckpoint: () => ipcRenderer.invoke('water:openCheckpoint'),
  saveWaterCheckpoint: (epoch) => ipcRenderer.invoke('water:saveCheckpoint', epoch),
  inspectWaterBudget: (epoch) => ipcRenderer.invoke('water:inspectBudget', epoch),
  saveResolvedWorld: (epoch) => ipcRenderer.invoke('world:saveResolved', epoch),
};
contextBridge.exposeInMainWorld('desktop', api);
