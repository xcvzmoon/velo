export type OAuthPending = { id: string; redirect_uri: string | null };
export type OAuthToken = {
  reference: string;
  expires_in: number | null;
  refresh_available: boolean;
};

type OAuthCancellation = {
  task: Promise<void> | null;
  failed: boolean;
};

/**
 * Owns one native OAuth flow until completion or cancellation.
 * @param beginning - The explicit native acquisition request, which may still be reserving its callback.
 * @param complete - Receives the native result without exposing token contents.
 * @param cancel - Cancels and waits for native cleanup for the matching flow identity.
 * @param signal - Aborted when the user cancels, closes the tab, or deactivates its cached view.
 * @param pending - Displays callback information only while the operation remains active.
 * @returns A secret reference, or null after cancellation; late results are never applied.
 * @throws Native acquisition/completion failures, or OAUTH_CANCEL_FAILED if cleanup cannot be confirmed.
 * @example
 * ```ts
 * const token = await receiveOAuthToken(beginning, complete, cancel, controller.signal, showPending)
 * if (token) applyReference(token.reference)
 * ```
 */
export async function receiveOAuthToken(
  beginning: Promise<OAuthPending>,
  complete: (id: string) => Promise<OAuthToken>,
  cancel: (id: string) => Promise<void>,
  signal: AbortSignal,
  pending: (flow: OAuthPending) => void,
): Promise<OAuthToken | null> {
  const flow = await beginning;
  if (signal.aborted) {
    await cancel(flow.id);
    return null;
  }
  const cancellation: OAuthCancellation = {
    task: null,
    failed: false,
  };
  let token: OAuthToken | null = null;
  function abort(): void {
    cancellation.task = cancel(flow.id).catch(() => {
      cancellation.failed = true;
    });
  }
  signal.addEventListener('abort', abort, { once: true });
  try {
    pending(flow);
    token = await complete(flow.id);
  } catch (error) {
    if (!signal.aborted) {
      await cancel(flow.id);
      throw error;
    }
  } finally {
    signal.removeEventListener('abort', abort);
    if (cancellation.task) {
      await cancellation.task;
    }
  }
  if (cancellation.failed) {
    throw new Error('OAUTH_CANCEL_FAILED: Native OAuth cleanup could not be confirmed.');
  }
  return signal.aborted ? null : token;
}
