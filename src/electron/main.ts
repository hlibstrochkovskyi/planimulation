import { app, BrowserWindow, dialog, ipcMain, Menu, session } from 'electron';
import type { IpcMainInvokeEvent } from 'electron';
import { open, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseRecipe, serializeRecipe } from '../core/recipe';

const rendererFile = path.join(__dirname, '../renderer/index.html');
const rendererURL = pathToFileURL(rendererFile).href;
let mainWindow: BrowserWindow | null = null;

function senderWindow(event: IpcMainInvokeEvent): BrowserWindow {
  if (!mainWindow || event.sender !== mainWindow.webContents || event.senderFrame !== event.sender.mainFrame
    || event.senderFrame.url !== rendererURL) throw new Error('Untrusted recipe request.');
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
  mainWindow.once('ready-to-show', () => mainWindow?.show());
  mainWindow.on('closed', () => { mainWindow = null; });
  void mainWindow.loadFile(rendererFile);
}

app.setName('Planimulation');
void app.whenReady().then(() => {
  session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  session.defaultSession.setPermissionCheckHandler(() => false);
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
  createWindow();
  app.on('activate', () => { if (BrowserWindow.getAllWindows().length === 0) createWindow(); });
});
app.on('window-all-closed', () => { if (process.platform !== 'darwin') app.quit(); });
