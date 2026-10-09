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
  seasonalTemperature: (epoch) => ipcRenderer.invoke('world:seasonalTemperature', epoch),
  seasonalWind: (epoch) => ipcRenderer.invoke('world:seasonalWind', epoch),
  seasonalMoisture: (epoch, seconds) => ipcRenderer.invoke('world:seasonalMoisture', epoch, seconds),
  initializeOrographicMoisture: (epoch) => ipcRenderer.invoke('world:initializeOrographicMoisture', epoch),
  initializePreciseMoisture: (epoch) => ipcRenderer.invoke('world:initializePreciseMoisture', epoch),
  initializeRegionalMoisture: (epoch) => ipcRenderer.invoke('world:initializeRegionalMoisture', epoch),
  initializeSoilMoisture: (epoch) => ipcRenderer.invoke('world:initializeSoilMoisture', epoch),
  seasonalSoilMoisture: (epoch, seconds) => ipcRenderer.invoke('world:seasonalSoilMoisture', epoch, seconds),
  openSeasonalCheckpoint: () => ipcRenderer.invoke('seasonal:openCheckpoint'),
  saveSeasonalCheckpoint: (epoch) => ipcRenderer.invoke('seasonal:saveCheckpoint', epoch),
  prescribeWater: (epoch, region, mode) => ipcRenderer.invoke('world:prescribeWater', epoch, region, mode),
  openWaterCheckpoint: () => ipcRenderer.invoke('water:openCheckpoint'),
  saveWaterCheckpoint: (epoch) => ipcRenderer.invoke('water:saveCheckpoint', epoch),
  inspectWaterBudget: (epoch) => ipcRenderer.invoke('water:inspectBudget', epoch),
  saveResolvedWorld: (epoch) => ipcRenderer.invoke('world:saveResolved', epoch),
};
contextBridge.exposeInMainWorld('desktop', api);
