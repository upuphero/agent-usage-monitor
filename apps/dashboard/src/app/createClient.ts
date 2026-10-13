import type { UsageClient } from '../api/client';
import { SyntheticClient } from '../api/transports/mock/SyntheticClient';
export async function createClient(native = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window): Promise<UsageClient> {
  if (!native) return new SyntheticClient();
  const { TauriClient } = await import('../api/transports/tauri/TauriClient');
  return new TauriClient();
}
