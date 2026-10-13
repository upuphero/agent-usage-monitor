import fixtures from '../../../../../../tests/fixtures/demo.json';
import { parseDashboard, type UsageClient } from '../../client';
import type { DashboardQuery } from '../../generated/monitor';
export class SyntheticClient implements UsageClient {
  readonly localView = false;
  readonly supportsLocalSync = false;
  async getDashboard(query: DashboardQuery) {
    if (query.scenario === 'local') throw new Error('Local logs require the desktop client');
    return parseDashboard(structuredClone(fixtures[query.scenario]));
  }
  async syncLocal(): Promise<never> { throw new Error('Local logs require the desktop client'); }
  async cancelLocalSync() { }
  async setLocalRoots(): Promise<never> { throw new Error('Local logs require the desktop client'); }
}
