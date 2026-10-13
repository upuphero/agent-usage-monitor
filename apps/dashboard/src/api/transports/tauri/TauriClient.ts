import { invoke, isTauri } from '@tauri-apps/api/core';
import { COMMANDS, type DashboardQuery, type SyncReport } from '../../generated/monitor';
import { parseDashboard, type UsageClient } from '../../client';
export { isTauri };
export class TauriClient implements UsageClient {
  readonly localView = true;
  readonly supportsLocalSync = true;
  async getDashboard(query: DashboardQuery) { return parseDashboard(await invoke(COMMANDS.getDashboard, { query })); }
  async syncLocal() { return invoke<SyncReport>(COMMANDS.syncLocal); }
  async cancelLocalSync() { await invoke(COMMANDS.cancelLocalSync); }
  async setLocalRoots(roots: string[]) { await invoke(COMMANDS.setLocalRoots, { request: { roots } }); }
}
