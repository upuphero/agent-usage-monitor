import fixtures from '../../../../../../tests/fixtures/demo.json';
import { parseDashboard, type UsageClient } from '../../client';
import type { DashboardQuery } from '../../generated/monitor';
export class SyntheticClient implements UsageClient {
  async getDashboard(query: DashboardQuery) { return parseDashboard(structuredClone(fixtures[query.scenario])); }
}
