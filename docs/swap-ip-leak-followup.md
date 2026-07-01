# Swap IP Exposure Follow-Up

## Context

The swap subsystem uses `crates/zbag-network/src/near_intents.rs`, whose default base URL is `https://1click.chaindefuser.com`. Requesting a quote, initiating a swap, and polling swap status contact that third-party service.

The existing Tor toggle and Tor state surface are the correct network controls. When Tor is off, swap calls are direct and the user's IP address is visible to 1Click.

## Follow-Up

Owner: frontend.

Add explicit copy to swap quote, initiation, and status-polling UI surfaces:

> Requesting a swap quote contacts a third-party service (1Click). Without Tor, your IP address is visible to that service.

This is documentation-only for now. No code change is included in the CEF network-hardening plan.
