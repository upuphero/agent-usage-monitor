import { parseDashboard, type UsageClient } from '../../client';
import type { DashboardQuery } from '../../generated/monitor';
import { SyntheticClient } from '../mock/SyntheticClient';
/** Explicit developer preview of a whitelisted CLI report, never an arbitrary filesystem read. */
export class LocalReportClient implements UsageClient {
  readonly localView = true;
  readonly supportsLocalSync = false;
  async getDashboard(query: DashboardQuery) {
    if (query.scenario !== 'local') return new SyntheticClient().getDashboard(query);
    const response = await fetch('/api/local-report', { cache: 'no-store' });
    if (!response.ok) throw new Error('Local preview report unavailable');
    const data = parseDashboard(await response.json());
    if (data.dataMode !== 'observed') throw new Error('Observed report required');
    return data;
  }
  async syncLocal(): Promise<never> { throw new Error('Read-only preview; use the desktop client to synchronize'); }
  async cancelLocalSync() { }
  async setLocalRoots(): Promise<never> { throw new Error('Use the desktop client to configure roots'); }
}
