import { app, BrowserWindow, dialog, ipcMain, Menu, session } from 'electron';
import type { IpcMainInvokeEvent } from 'electron';
import { open, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseRecipe, serializeRecipe } from '../core/recipe';
import { NativeController } from '../native/client';
import { writeResolvedWorld } from '../native/resolved-export';
import { parseCaptureRect } from '../shared/capture';
import { MAX_SEASONAL_CHECKPOINT_BYTES } from '../shared/seasonal-checkpoint';
import { SOIL_MOISTURE_CONTRACT } from '../shared/soil-moisture';
import { readCheckpoint, writeCheckpoint } from './checkpoint-files';

const rendererFile = path.join(__dirname, '../renderer/index.html');
const rendererURL = pathToFileURL(rendererFile).href;
const MAX_CHECKPOINT_BYTES = 8 * 2 ** 20;
let mainWindow: BrowserWindow | null = null;
const core = new NativeController(path.join(app.isPackaged ? process.resourcesPath : path.join(__dirname, '..'),
  'native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));

function senderWindow(event: IpcMainInvokeEvent): BrowserWindow {
  if (!mainWindow || event.sender !== mainWindow.webContents || event.senderFrame !== event.sender.mainFrame
    || event.senderFrame.url !== rendererURL) throw new Error('Untrusted renderer request.');
  return mainWindow;
}

function createWindow(): void {
  mainWindow = new BrowserWindow({
    title: 'Planimulation', width: 1440, height: 940, minWidth: 1000, minHeight: 700,
    backgroundColor: '#111315', show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true, sandbox: true, nodeIntegration: false, webSecurity: true,
    },
  });
  if (process.platform !== 'darwin') mainWindow.setMenuBarVisibility(false);
  mainWindow.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  mainWindow.webContents.on('will-navigate', (event) => event.preventDefault());
  mainWindow.webContents.on('did-start-loading', () => core.close());
  mainWindow.webContents.on('render-process-gone', () => core.close());
  mainWindow.once('ready-to-show', () => mainWindow?.show());
  mainWindow.on('closed', () => { core.close(); mainWindow = null; });
  void mainWindow.loadFile(rendererFile);
}

app.setName('Planimulation');
void app.whenReady().then(() => {
  session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  session.defaultSession.setPermissionCheckHandler(() => false);
  ipcMain.handle('world:generate', (event, recipe: unknown) => { senderWindow(event); return core.generate(parseRecipe(recipe)); });
  ipcMain.handle('world:cancel', (event) => { senderWindow(event); core.cancel(); });
  ipcMain.handle('world:accept', (event, epoch: number) => { senderWindow(event); core.accept(epoch); });
  ipcMain.handle('world:advance', (event, epoch: number, steps: number) => { senderWindow(event); return core.advance(epoch, steps); });
  ipcMain.handle('world:seasonalTemperature', (event, epoch: number) => { senderWindow(event); return core.seasonalTemperature(epoch); });
  ipcMain.handle('world:seasonalWind', (event, epoch: number) => { senderWindow(event); return core.seasonalWind(epoch); });
  ipcMain.handle('world:seasonalMoisture', (event, epoch: number, seconds: number) => {
    senderWindow(event); return core.seasonalMoisture(epoch, seconds);
  });
  ipcMain.handle('world:initializeOrographicMoisture', (event, epoch: number) => {
    senderWindow(event); return core.initializeOrographicMoisture(epoch);
  });
  ipcMain.handle('world:initializePreciseMoisture', (event, epoch: number) => {
    senderWindow(event); return core.initializePreciseMoisture(epoch);
  });
  ipcMain.handle('world:initializeRegionalMoisture', (event, epoch: number) => {
    senderWindow(event); return core.initializeRegionalMoisture(epoch);
  });
  ipcMain.handle('world:prescribeWater', (event, epoch: number, region: number, mode: unknown) => {
    senderWindow(event);
    if (mode !== 'oneCubicKilometer' && mode !== 'fillToSpill') throw new Error('Invalid prescribed-water mode.');
    return core.prescribeWater(epoch, region, mode);
  });
  ipcMain.handle('world:initializeSoilMoisture', (event, epoch: number) => {
    senderWindow(event); return core.initializeSoilMoisture(epoch);
  });
  ipcMain.handle('world:seasonalSoilMoisture', (event, epoch: number, seconds: number) => {
    senderWindow(event); return core.seasonalSoilMoisture(epoch, seconds);
  });
  ipcMain.handle('water:openCheckpoint', async (event) => {
    const window = senderWindow(event);
    const revision = core.preparationRevision;
    const result = await dialog.showOpenDialog(window, { properties: ['openFile'],
      filters: [{ name: 'Prescribed-water checkpoint', extensions: ['json'] }] });
    if (result.canceled) return null;
    const contents = await readCheckpoint(result.filePaths[0], MAX_CHECKPOINT_BYTES, 'Water checkpoint');
    if (revision !== core.preparationRevision) throw new Error('Checkpoint opening was canceled or replaced.');
    return core.loadWaterCheckpoint(JSON.parse(contents));
  });
  ipcMain.handle('water:saveCheckpoint', async (event, epoch: number) => {
    const window = senderWindow(event);
    const result = await dialog.showSaveDialog(window, { defaultPath: 'water-checkpoint.json',
      filters: [{ name: 'Prescribed-water checkpoint', extensions: ['json'] }] });
    if (result.canceled || !result.filePath) return false;
    const contents = await core.exportWaterCheckpoint(epoch);
    await writeCheckpoint(result.filePath, contents);
    return true;
  });
  ipcMain.handle('seasonal:openCheckpoint', async (event) => {
    const window = senderWindow(event), revision = core.preparationRevision;
    const result = await dialog.showOpenDialog(window, { properties: ['openFile'],
      filters: [{ name: 'Seasonal-water checkpoint', extensions: ['json'] }] });
    if (result.canceled) return null;
    const contents = await readCheckpoint(result.filePaths[0], MAX_SEASONAL_CHECKPOINT_BYTES, 'Seasonal checkpoint');
    if (revision !== core.preparationRevision) throw new Error('Checkpoint opening was canceled or replaced.');
    const metadata = JSON.parse(contents) as { modelVersion?: unknown } | null;
    return metadata?.modelVersion === SOIL_MOISTURE_CONTRACT.modelVersion
      ? core.loadSoilCheckpoint(contents) : core.loadSeasonalCheckpoint(contents);
  });
  ipcMain.handle('seasonal:saveCheckpoint', async (event, epoch: number) => {
    const window = senderWindow(event), world = core.resolvedInitialWorld(epoch);
    // Snapshot before the dialog; subsequent file I/O cannot change its clock.
    const contents = await core.exportSeasonalCheckpoint(epoch);
    const result = await dialog.showSaveDialog(window, { defaultPath: 'seasonal-water-checkpoint.json',
      filters: [{ name: 'Seasonal-water checkpoint', extensions: ['json'] }] });
    if (result.canceled || !result.filePath) return false;
    if (core.resolvedInitialWorld(epoch) !== world) throw new Error('World changed during seasonal checkpoint export.');
    await writeCheckpoint(result.filePath, contents);
    return true;
  });
  ipcMain.handle('water:inspectBudget', (event, epoch: number) => {
    senderWindow(event);
    return core.inspectWaterBudget(epoch);
  });
  ipcMain.handle('world:saveResolved', async (event, epoch: number) => {
    const window = senderWindow(event);
    const world = core.resolvedInitialWorld(epoch);
    const result = await dialog.showSaveDialog(window, { defaultPath: 'resolved-initial-world.json',
      filters: [{ name: 'Resolved initial world', extensions: ['json'] }] });
    if (result.canceled || !result.filePath) return false;
    if (core.resolvedInitialWorld(epoch) !== world) throw new Error('World changed during resolved-state export.');
    await writeResolvedWorld(result.filePath, world);
    return true;
  });
  Menu.setApplicationMenu(Menu.buildFromTemplate([
    ...(process.platform === 'darwin' ? [{ role: 'appMenu' as const }] : []),
    { label: 'File', submenu: [{ role: 'quit' }] },
    { label: 'Edit', submenu: [{ role: 'undo' }, { role: 'redo' }, { type: 'separator' }, { role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' }] },
    { label: 'View', submenu: [{ role: 'reload' }, { role: 'toggleDevTools' }, { role: 'togglefullscreen' }] },
  ]));
  ipcMain.handle('recipe:open', async (event) => {
    const window = senderWindow(event);
    const result = await dialog.showOpenDialog(window, { properties: ['openFile'], filters: [{ name: 'World recipe', extensions: ['json'] }] });
    if (result.canceled) return null;
    const file = await open(result.filePaths[0], 'r');
    try {
      if ((await file.stat()).size > 32_768) throw new Error('Recipe exceeds the 32 KiB limit.');
      // A bounded read also protects against a file growing between stat and read.
      const buffer = Buffer.alloc(32_769);
      const { bytesRead } = await file.read(buffer, 0, buffer.length, 0);
      if (bytesRead > 32_768) throw new Error('Recipe exceeds the 32 KiB limit.');
      return parseRecipe(JSON.parse(buffer.subarray(0, bytesRead).toString('utf8')));
    } finally { await file.close(); }
  });
  ipcMain.handle('recipe:save', async (event, value: unknown) => {
    const window = senderWindow(event);
    const contents = serializeRecipe(parseRecipe(value));
    const result = await dialog.showSaveDialog(window, { defaultPath: 'world-recipe.json', filters: [{ name: 'World recipe', extensions: ['json'] }] });
    if (result.canceled || !result.filePath) return false;
    await writeFile(result.filePath, contents, 'utf8');
    return true;
  });
  ipcMain.handle('view:export', async (event, value: unknown) => {
    const window = senderWindow(event);
    const bounds = window.getContentBounds();
    const rect = parseCaptureRect(value, bounds);
    const result = await dialog.showSaveDialog(window, { defaultPath: 'world-view.png', filters: [{ name: 'PNG image', extensions: ['png'] }] });
    if (result.canceled || !result.filePath) return false;
    const image = await window.webContents.capturePage(rect);
    if (image.isEmpty()) throw new Error('The map view could not be captured.');
    const png = image.toPNG();
    if (png.length > 32 * 1024 * 1024) throw new Error('The map image exceeds the 32 MiB export limit.');
    await writeFile(result.filePath, png);
    return true;
  });
  createWindow();
  app.on('activate', () => { if (BrowserWindow.getAllWindows().length === 0) createWindow(); });
});
app.on('window-all-closed', () => { if (process.platform !== 'darwin') app.quit(); });
app.on('before-quit', () => core.close());
