import type { OAuthPending, OAuthToken } from '../../../app/utils/oauthFlow';
import { describe, expect, it } from 'vitest';
import { receiveOAuthToken } from '../../../app/utils/oauthFlow';

const token: OAuthToken = {
  reference: '{{secret:oauth.test}}',
  expires_in: 60,
  refresh_available: true,
};
const flow: OAuthPending = {
  id: 'flow',
  redirect_uri: 'http://127.0.0.1:1234/oauth/callback',
};

describe('owned OAuth frontend lifecycle', () => {
  it('cancels a delayed native begin after tab deactivation without displaying or applying it', async () => {
    const beginning = Promise.withResolvers<OAuthPending>();
    const controller = new AbortController();
    const cancelled: string[] = [];
    const displayed: string[] = [];
    let completed = false;
    const result = receiveOAuthToken(
      beginning.promise,
      () => {
        completed = true;
        return Promise.resolve(token);
      },
      (id) => {
        cancelled.push(id);
        return Promise.resolve();
      },
      controller.signal,
      (pending) => {
        displayed.push(pending.id);
      },
    );
    controller.abort();
    beginning.resolve(flow);
    expect(await result).toBeNull();
    expect(cancelled).toEqual(['flow']);
    expect(displayed).toEqual([]);
    expect(completed).toBe(false);
  });

  it('suppresses late completion and awaits cleanup after cached tab deactivation', async () => {
    const completion = Promise.withResolvers<OAuthToken>();
    const cleanup = Promise.withResolvers<undefined>();
    const displayed = Promise.withResolvers<undefined>();
    const controller = new AbortController();
    const cancelled: string[] = [];
    let settled = false;
    const result = receiveOAuthToken(
      Promise.resolve(flow),
      () => completion.promise,
      async (id) => {
        cancelled.push(id);
        await cleanup.promise;
      },
      controller.signal,
      () => {
        displayed.resolve(undefined);
      },
    );
    void result.then(() => {
      settled = true;
    });
    await displayed.promise;
    controller.abort();
    completion.resolve(token);
    await Promise.resolve();
    expect(settled).toBe(false);
    expect(cancelled).toEqual(['flow']);
    cleanup.resolve(undefined);
    expect(await result).toBeNull();
  });

  it('keeps a reactivated tab operation independent from the cancelled old completion', async () => {
    const oldCompletion = Promise.withResolvers<OAuthToken>();
    const displayed = Promise.withResolvers<undefined>();
    const old = new AbortController();
    const oldResult = receiveOAuthToken(
      Promise.resolve(flow),
      () => oldCompletion.promise,
      () => Promise.resolve(),
      old.signal,
      () => {
        displayed.resolve(undefined);
      },
    );
    await displayed.promise;
    old.abort();
    const next = new AbortController();
    const nextResult = receiveOAuthToken(
      Promise.resolve({
        ...flow,
        id: 'next',
      }),
      () => Promise.resolve(token),
      () => Promise.resolve(),
      next.signal,
      () => undefined,
    );
    expect(await nextResult).toEqual(token);
    oldCompletion.resolve(token);
    expect(await oldResult).toBeNull();
  });

  it('cancels native work on completion errors and reports cleanup failures', async () => {
    const controller = new AbortController();
    const cancelled: string[] = [];
    await expect(
      receiveOAuthToken(
        Promise.resolve(flow),
        () => Promise.reject(new Error('OAUTH_FAILED')),
        (id) => {
          cancelled.push(id);
          return Promise.resolve();
        },
        controller.signal,
        () => undefined,
      ),
    ).rejects.toThrow('OAUTH_FAILED');
    expect(cancelled).toEqual(['flow']);
    const completion = Promise.withResolvers<OAuthToken>();
    const displayed = Promise.withResolvers<undefined>();
    const result = receiveOAuthToken(
      Promise.resolve(flow),
      () => completion.promise,
      () => Promise.reject(new Error('native cleanup failed')),
      controller.signal,
      () => {
        displayed.resolve(undefined);
      },
    );
    await displayed.promise;
    controller.abort();
    completion.resolve(token);
    await expect(result).rejects.toThrow('OAUTH_CANCEL_FAILED');
  });
});
