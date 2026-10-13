import type { UsageClient } from '../api/client';
import { SyntheticClient } from '../api/transports/mock/SyntheticClient';
export const isDesktopRuntime = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
export const isLocalReportPreview = () => typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('local-report') === '1';
export async function createClient(native = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window): Promise<UsageClient> {
  if (!native) {
    if (isLocalReportPreview()) { const { LocalReportClient } = await import('../api/transports/local-report/LocalReportClient'); return new LocalReportClient(); }
    return new SyntheticClient();
  }
  const { TauriClient } = await import('../api/transports/tauri/TauriClient');
  return new TauriClient();
}
