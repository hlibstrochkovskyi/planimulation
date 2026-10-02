import { app, BrowserWindow, dialog, ipcMain, Menu, session } from 'electron';
import type { IpcMainInvokeEvent } from 'electron';
import { randomUUID } from 'node:crypto';
import { open, rename, unlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseRecipe, serializeRecipe } from '../core/recipe';
import { NativeController } from '../native/client';
import { parseCaptureRect } from '../shared/capture';

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
    backgroundColor: '#101719', show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true, sandbox: true, nodeIntegration: false, webSecurity: true,
    },
  });
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
  ipcMain.handle('world:prescribeWater', (event, epoch: number, region: number, mode: unknown) => {
    senderWindow(event);
    if (mode !== 'oneCubicKilometer' && mode !== 'fillToSpill') throw new Error('Invalid prescribed-water mode.');
    return core.prescribeWater(epoch, region, mode);
  });
  ipcMain.handle('water:openCheckpoint', async (event) => {
    const window = senderWindow(event);
    const result = await dialog.showOpenDialog(window, { properties: ['openFile'],
      filters: [{ name: 'Prescribed-water checkpoint', extensions: ['json'] }] });
    if (result.canceled) return null;
    const file = await open(result.filePaths[0], 'r');
    try {
      if ((await file.stat()).size > MAX_CHECKPOINT_BYTES) throw new Error('Water checkpoint exceeds the 8 MiB limit.');
      const buffer = Buffer.alloc(MAX_CHECKPOINT_BYTES + 1);
      let count = 0;
      while (count < buffer.length) {
        const { bytesRead } = await file.read(buffer, count, buffer.length - count, count);
        if (bytesRead === 0) break;
        count += bytesRead;
      }
      if (count > MAX_CHECKPOINT_BYTES) throw new Error('Water checkpoint exceeds the 8 MiB limit.');
      return await core.loadWaterCheckpoint(JSON.parse(buffer.subarray(0, count).toString('utf8')));
    } finally { await file.close(); }
  });
  ipcMain.handle('water:saveCheckpoint', async (event, epoch: number) => {
    const window = senderWindow(event);
    const result = await dialog.showSaveDialog(window, { defaultPath: 'water-checkpoint.json',
      filters: [{ name: 'Prescribed-water checkpoint', extensions: ['json'] }] });
    if (result.canceled || !result.filePath) return false;
    const contents = await core.exportWaterCheckpoint(epoch);
    const temporary = `${result.filePath}.tmp-${randomUUID()}`;
    try {
      await writeFile(temporary, contents, { encoding: 'utf8', flag: 'wx' });
      await rename(temporary, result.filePath);
    } catch (error) {
      await unlink(temporary).catch(() => {});
      throw error;
    }
    return true;
  });
  ipcMain.handle('water:inspectBudget', (event, epoch: number) => {
    senderWindow(event);
    return core.inspectWaterBudget(epoch);
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
