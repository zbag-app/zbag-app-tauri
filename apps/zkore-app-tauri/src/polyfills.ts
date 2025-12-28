// Some third-party deps (transitively via Keystone libs) assume a Node-style `process` global exists.
// Tauri WebViews don't provide it, so we add a minimal shim to prevent a startup crash.

import { Buffer } from "buffer/";

const globalAny = globalThis as any;

if (typeof globalAny.global === "undefined") {
  globalAny.global = globalAny;
}

if (typeof globalAny.Buffer === "undefined") {
  globalAny.Buffer = Buffer;
}

if (typeof globalAny.process === "undefined") {
  globalAny.process = {
    argv: [],
    browser: true,
    cwd: () => "/",
    env: { NODE_ENV: import.meta.env.MODE },
    pid: 0,
    title: "browser",
    version: "",
    versions: {},
    nextTick: (callback: (...args: any[]) => void, ...args: any[]) => {
      const run = () => callback(...args);
      if (typeof queueMicrotask === "function") {
        queueMicrotask(run);
        return;
      }
      Promise.resolve().then(run);
    },
    on: () => {},
    addListener: () => {},
    once: () => {},
    off: () => {},
    removeListener: () => {},
    removeAllListeners: () => {},
    emit: () => false,
    stdout: undefined,
    stderr: undefined,
    chdir: () => {
      throw new Error("process.chdir is not supported");
    },
    umask: () => 0,
  };
} else {
  if (globalAny.process.env == null) globalAny.process.env = {};
  if (globalAny.process.env.NODE_ENV == null) globalAny.process.env.NODE_ENV = import.meta.env.MODE;
  if (globalAny.process.browser == null) globalAny.process.browser = true;
  if (globalAny.process.version == null) globalAny.process.version = "";
  if (globalAny.process.versions == null) globalAny.process.versions = {};
}
