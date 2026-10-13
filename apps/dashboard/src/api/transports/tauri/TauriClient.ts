import { invoke, isTauri } from '@tauri-apps/api/core';
import { COMMANDS, type DashboardQuery } from '../../generated/monitor';
import { parseDashboard, type UsageClient } from '../../client';
export { isTauri };
export class TauriClient implements UsageClient {
  async getDashboard(query: DashboardQuery) { return parseDashboard(await invoke(COMMANDS.getDashboard, { query })); }
}
