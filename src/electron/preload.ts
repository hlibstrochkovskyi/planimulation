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
};
contextBridge.exposeInMainWorld('desktop', api);
