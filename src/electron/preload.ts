import { contextBridge, ipcRenderer } from 'electron';
import type { DesktopAPI } from '../shared/desktop-api';

const api: DesktopAPI = {
  openRecipe: () => ipcRenderer.invoke('recipe:open'),
  saveRecipe: (recipe) => ipcRenderer.invoke('recipe:save', recipe),
};
contextBridge.exposeInMainWorld('desktop', api);
