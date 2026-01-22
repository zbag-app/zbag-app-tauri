import { useCallback, useEffect, useState } from 'react';
import type * as IPC from '../types/ipc';
import { onTorStatus } from '../services/events';
import { getExchangeRate, getFiatSettings, setFiatSettings } from '../services/ipc';

interface FiatDisplayState {
  settings: IPC.FiatDisplaySettings | null;
  rate: IPC.ExchangeRate | null;
  isStale: boolean;
  refreshCooldownSecs: number;
  loading: boolean;
  error: string | null;
  /** Error from periodic background refresh (does not prevent display of stale data) */
  refreshError: string | null;
  /** Retry attempt counter for exponential backoff */
  retryAttempt: number;
}

export function useFiatDisplay() {
  const [state, setState] = useState<FiatDisplayState>({
    settings: null,
    rate: null,
    isStale: true,
    refreshCooldownSecs: 0,
    loading: true,
    error: null,
    refreshError: null,
    retryAttempt: 0,
  });

  // Extract enabled to a stable primitive to avoid unnecessary effect re-runs
  const enabled = state.settings?.enabled ?? false;

  const fetchRate = useCallback(async (options?: { force?: boolean; signal?: AbortSignal }) => {
    const force = options?.force ?? false;
    const signal = options?.signal;
    const res = await getExchangeRate(force ? { force_refresh: true } : {});

    // Check abort before updating state
    if (signal?.aborted) return null;

    if ('ok' in res) {
      const rate = res.ok.rate;
      setState((prev) => ({
        ...prev,
        // Preserve an existing rate if the backend can't return one right now,
        // but only when it matches the currently-selected currency.
        rate:
          rate ??
          (prev.rate && prev.settings && prev.rate.currency === prev.settings.currency ? prev.rate : null),
        isStale: rate ? res.ok.is_stale : true,
        refreshCooldownSecs: res.ok.refresh_cooldown_secs,
        refreshError: rate
          ? null
          : res.ok.refresh_cooldown_secs > 0
            ? `Exchange rate temporarily unavailable. Retry in ${res.ok.refresh_cooldown_secs}s.`
            : 'Exchange rate temporarily unavailable.',
        // Reset retry counter on successful fetch with rate, otherwise increment for backoff
        retryAttempt: rate ? 0 : prev.retryAttempt + 1,
      }));
      return rate;
    }

    // Clear rate on failure to prevent displaying wrong-currency values after a currency change
    setState((prev) => ({
      ...prev,
      rate: null,
      isStale: true,
      refreshError: res.err.message,
      retryAttempt: prev.retryAttempt + 1,
    }));
    return null;
  }, []);

  const loadSettings = useCallback(async () => {
    setState((prev) => ({ ...prev, loading: true }));
    const res = await getFiatSettings();
    if ('err' in res) {
      setState((prev) => ({ ...prev, error: res.err.message, loading: false }));
      return;
    }
    setState((prev) => ({ ...prev, settings: res.ok.settings, error: null }));

    // If fiat is enabled, also load the rate
    if (res.ok.settings.enabled) {
      await fetchRate();
      setState((prev) => ({ ...prev, loading: false }));
    } else {
      setState((prev) => ({
        ...prev,
        rate: null,
        isStale: true,
        refreshCooldownSecs: 0,
        refreshError: null,
        retryAttempt: 0,
        loading: false,
      }));
    }
  }, [fetchRate]);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // Refresh rate periodically when enabled (every 5 minutes)
  useEffect(() => {
    if (!enabled) return;

    const controller = new AbortController();

    const interval = setInterval(() => {
      fetchRate({ signal: controller.signal }).catch(() => {});
    }, 5 * 60 * 1000);

    return () => {
      controller.abort();
      clearInterval(interval);
    };
  }, [enabled, fetchRate]);

  // If fiat is enabled but we don't have a rate yet, retry with exponential backoff.
  useEffect(() => {
    if (!enabled) return;
    if (state.rate) return;

    const controller = new AbortController();

    const baseDelaySecs = 10;
    const maxDelaySecs = 300; // 5 minutes
    // Use cooldown from backend if set, otherwise apply exponential backoff
    const delaySecs =
      state.refreshCooldownSecs > 0
        ? state.refreshCooldownSecs
        : Math.min(baseDelaySecs * Math.pow(2, state.retryAttempt), maxDelaySecs);
    const timeout = setTimeout(() => {
      fetchRate({ signal: controller.signal }).catch(() => {});
    }, delaySecs * 1000);

    return () => {
      controller.abort();
      clearTimeout(timeout);
    };
  }, [enabled, state.rate, state.refreshCooldownSecs, state.retryAttempt, fetchRate]);

  // When Tor becomes ready, immediately retry (common case: fiat enabled while Tor is connecting).
  useEffect(() => {
    if (!enabled) return;
    if (state.rate) return;

    const controller = new AbortController();
    let unlisten: (() => void) | null = null;

    onTorStatus((evt) => {
      if (controller.signal.aborted) return;
      if (!evt.state.enabled) return;
      if (evt.state.status !== 'On') return;
      fetchRate({ signal: controller.signal }).catch(() => {});
    })
      .then((fn) => {
        if (!controller.signal.aborted) {
          unlisten = fn;
        } else {
          // Component unmounted before promise resolved - clean up immediately
          fn();
        }
      })
      .catch(() => {});

    return () => {
      controller.abort();
      unlisten?.();
    };
  }, [enabled, state.rate, fetchRate]);

  const updateSettings = useCallback(
    async (enabled: boolean, currency: IPC.FiatCurrency, privacyAcknowledged: boolean) => {
      setState((prev) => ({ ...prev, loading: true, error: null }));

      const res = await setFiatSettings({
        enabled,
        currency,
        privacy_acknowledged: privacyAcknowledged,
      });

      if ('err' in res) {
        setState((prev) => ({ ...prev, error: res.err.message, loading: false }));
        return false;
      }

      setState((prev) => ({ ...prev, settings: res.ok.settings, error: null }));

      // If enabled, fetch the rate
      if (enabled) {
        await fetchRate();
        setState((prev) => ({ ...prev, loading: false }));
      } else {
        setState((prev) => ({
          ...prev,
          rate: null,
          isStale: true,
          refreshCooldownSecs: 0,
          refreshError: null,
          retryAttempt: 0,
          loading: false,
        }));
      }

      return true;
    },
    [fetchRate]
  );

  const refreshRate = useCallback(async (force = false) => {
    await fetchRate({ force });
  }, [fetchRate]);

  return {
    settings: state.settings,
    rate: state.rate,
    isStale: state.isStale,
    refreshCooldownSecs: state.refreshCooldownSecs,
    loading: state.loading,
    error: state.error,
    refreshError: state.refreshError,
    updateSettings,
    refreshRate,
    reload: loadSettings,
  };
}
