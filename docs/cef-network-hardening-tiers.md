# CEF Network Hardening Tiers

## Promotion Gate

Tier 2 feature names and Tier 3 `--user-data-dir` pinning are documentation-only. Do not add them to `cef_runtime_args` unless the Layer 3 runtime smoke observes a real non-loopback leak that Tier 1 does not block. The goal is to avoid preemptive Chromium flag churn while keeping a ready escalation path.

## Tier 1: Current, Shipped

Plain switches:

- `--disable-background-networking`: disables Chromium background services.
- `--disable-breakpad`: disables Chromium crash-reporting paths.
- `--disable-component-extensions-with-background-pages`: blocks bundled background extension pages.
- `--disable-component-update`: disables component updater traffic.
- `--disable-default-apps`: avoids default Chromium app activation.
- `--disable-domain-reliability`: disables domain-reliability upload paths.
- `--disable-extensions`: disables extension loading.
- `--disable-field-trial-config`: disables field-trial config fetches.
- `--disable-notifications`: disables browser notification services.
- `--disable-print-preview`: removes print-preview browser surface.
- `--disable-save-password-bubble`: keeps password UX app-controlled.
- `--disable-speech-api`: disables speech-service browser surface.
- `--disable-sync`: disables Chromium sync.
- `--disable-sync-invalidation-optimizations`: disables sync invalidation paths.
- `--incognito`: avoids durable browser profile behavior.
- `--metrics-recording-only`: keeps metrics local only.
- `--no-default-browser-check`: disables default-browser probes.
- `--no-first-run`: disables first-run flows.
- `--no-pings`: disables hyperlink auditing pings.

Valued arguments:

- `--disable-features=AutofillActorMode,AutofillServerCommunication,AsyncDns,DnsOverHttpsUpgrade,EnableMediaRouter,GlicActorUi,LensOverlay,LiveTranslate,MediaRouter,OptimizationGuideModelExecution,OptimizationGuideOnDeviceModel,OptimizationHints,PrivacySandboxSettings4,Translate,UseDnsHttpsSvcb`
- `--dns-over-https-mode=off`
- `--dns-over-https-templates=`
- `--host-resolver-rules=MAP * 0.0.0.0,EXCLUDE localhost,EXCLUDE 127.0.0.1,EXCLUDE ::1,EXCLUDE *.localhost,EXCLUDE ipc.localhost,EXCLUDE tauri.localhost`
- `--webrtc-ip-handling-policy=disable_non_proxied_udp`

Preference hardening disables Safe Browsing, search suggestions, spell service, translation, sign-in, network prediction, Privacy Sandbox, autofill, password manager, DoH fallback, and WebRTC non-proxied UDP.

## Tier 2: Documented, Not Shipped

Only promote these if the runtime smoke proves Tier 1 is insufficient:

- `BrowsingTopics`
- `Fledge`
- `InterestGroupStorage`
- `AttributionReporting`
- `PrivateAggregationApi`
- `SharedStorageAPI`
- `FencedFrames`
- `NetworkTimeServiceQuerying`
- `NetworkQualityEstimator`
- `Reporting`
- `NetworkErrorLogging`
- `Prerender2`
- `Preconnect`
- `LoadingPredictorUseLocalPredictions`
- `PushMessaging`
- `BackgroundSync`
- `BackgroundFetch`
- `WidevineCdm`
- `DialMediaRouteProvider`
- `CastMediaRouteProvider`
- `MediaRemoting`
- `HttpsUpgrades`
- `WebBluetooth`
- `WebUsb`
- `WebHID`

Chromium silently ignores unknown feature names, so an over-broad list is usually runtime-safe. The downside is auditability: ignored or stale names make it harder to tell which switch is doing useful work.

## Tier 3: Documented, Not Shipped

Add an explicit `--user-data-dir=<cef_runtime_cache_path>` only if smoke evidence shows a non-loopback peer despite Tier 1. Some Chromium subsystems may consult `--user-data-dir` separately from Tauri's `root_cache_path`; pinning it to the same per-launch temp cache is a belt-and-suspenders option.

## Staged Peel-Back If Startup Breaks

1. Tier 1 current: all shipped switches, valued args, temp cache, and prefs hardening.
2. Tier 1 minus `--disable-features`: keep host resolver, DoH off, temp cache, and prefs. Validate with rebuild, app launch, and Little Snitch or `make cef-smoketest`.
3. Minimum viable network block: keep `--host-resolver-rules`, DoH off, per-launch cache, and prefs. Rebuild and validate again.
4. Cache and prefs only: use only as a temporary diagnostic state, never as a release candidate, because Chromium hostname resolution is no longer deny-all.

For each peel-back step, rebuild the packaged app, launch cold with isolated state, and confirm no Google, Cloudflare, OpenDNS, YouTube, or other Chromium service hosts appear in the bagZ process tree.

## Upstream Stability Fixes

The strict path depends on Tauri CEF fixes that are ancestors of the pinned Tauri rev `6fd733b2d6255d358e88ad19cb15dc7d22b293ac` from 2026-05-14:

- Tauri PR `#15252`: null pointer shutdown fix, merged 2026-04-16.
- Tauri PR `#15279`: user-event callback re-entrancy guard, merged 2026-04-28.

## Validation Procedure

Use A/B validation when changing tiers:

1. Build the app with the candidate policy.
2. Run `make cef-smoketest`.
3. Launch manually and inspect the process tree with Little Snitch or `lsof`.
4. Remove or add one tier at a time.
5. Repeat until the smallest stable policy still blocks non-loopback CEF traffic.
