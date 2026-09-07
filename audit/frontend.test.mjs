import { build } from "esbuild";
import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import React from "react";
import Renderer, { act } from "react-test-renderer";
mkdirSync(".tooling/tests", { recursive: true });
await build({
  entryPoints: ["audit/frontend-fixture.tsx"],
  bundle: true,
  platform: "node",
  format: "cjs",
  outfile: ".tooling/tests/frontend.cjs",
  external: ["react", "react/jsx-runtime"],
  plugins: [
    {
      name: "fake-native",
      setup(build) {
        build.onResolve({ filter: /^@tauri-apps\// }, (args) => ({
          path: args.path,
          namespace: "fake-native",
        }));
        build.onLoad({ filter: /.*/, namespace: "fake-native" }, () => ({
          contents:
            "export const invoke = (...args) => globalThis.__invoke(...args); export const open = async()=>{};",
        }));
      },
    },
  ],
});
const events = new EventTarget();
globalThis.window = {
  matchMedia: () => ({ matches: false }),
  addEventListener: events.addEventListener.bind(events),
  removeEventListener: events.removeEventListener.bind(events),
};
globalThis.localStorage = { getItem: () => null, setItem: () => {} };
const require = createRequire(import.meta.url);
const { WhistleView, RuleEditor, useSettingsForm, useAppStore, buildFallbackConfig } = require(
  resolve(".tooling/tests/frontend.cjs"),
);
const initial = useAppStore.getState();
let calls = [],
  alive = true,
  failCommand = null;
function reset(mode = "embedded") {
  const config = buildFallbackConfig();
  config.whistle.mode = mode;
  calls = [];
  alive = true;
  failCommand = null;
  useAppStore.setState({
    ...initial,
    config,
    loading: false,
    whistleStatus: {
      running: true,
      uptime_check: true,
      mode,
      host: "127.0.0.1",
      port: 18899,
      pid: 123,
    },
    authProxyUrl: "http://127.0.0.1:18900?_token=fixture",
  });
  globalThis.__invoke = async (cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === failCommand) throw new Error("fixture operation denied");
    if (cmd === "cmd_get_config") return useAppStore.getState().config;
    if (cmd === "cmd_get_whistle_status")
      return { ...useAppStore.getState().whistleStatus, running: alive, uptime_check: alive };
    if (cmd === "cmd_start_auth_proxy") return "http://127.0.0.1:18900?_token=fixture";
    if (cmd === "cmd_probe_auth_proxy") return true;
    if (cmd === "cmd_get_proxy_status") return { mode: "direct", enabled: false };
    return false;
  };
}
const settle = async () => {
  for (let i = 0; i < 30; i++) await Promise.resolve();
};
async function mount(Component) {
  let r;
  await act(async () => {
    r = Renderer.create(React.createElement(Component));
    await settle();
  });
  return r;
}
async function unmount(r) {
  await act(async () => {
    r.unmount();
    await settle();
  });
}

test("healthy Whistle iframe remains mounted for 18 seconds", async (t) => {
  reset();
  t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
  const r = await mount(WhistleView);
  try {
    const frame = r.root.findByType("iframe");
    for (let i = 0; i < 3; i++)
      await act(async () => {
        t.mock.timers.tick(6000);
        await settle();
      });
    assert.ok(r.root.findByType("iframe") === frame, "healthy iframe was remounted");
  } finally {
    await unmount(r);
  }
});
test("embedded Whistle view observes background startup", async (t) => {
  reset();
  alive = false;
  useAppStore.setState({
    whistleStatus: { ...useAppStore.getState().whistleStatus, running: false, uptime_check: false },
  });
  t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
  const r = await mount(WhistleView);
  try {
    alive = true;
    for (let i = 0; i < 8; i++)
      await act(async () => {
        t.mock.timers.tick(8000);
        await settle();
      });
    assert.equal(r.root.findAllByType("iframe").length, 1);
  } finally {
    await unmount(r);
  }
});
test("failed save and export reject to the caller", async () => {
  reset();
  failCommand = "cmd_save_config";
  await assert.rejects(useAppStore.getState().saveConfig(buildFallbackConfig()), /denied/);
  failCommand = "cmd_export_config";
  await assert.rejects(useAppStore.getState().exportConfig("fixture.json"), /denied/);
});
test("new rule is not lost when leaving before debounce ends", async (t) => {
  reset();
  t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
  const r = await mount(RuleEditor);
  await act(async () => {
    r.root.findAllByType("input")[0].props.onChange({ target: { value: "example.com" } });
  });
  await act(async () => {
    r.root.findAllByType("input")[0].props.onKeyDown({ key: "Enter" });
  });
  await unmount(r);
  await act(async () => {
    t.mock.timers.tick(1000);
    await settle();
  });
  assert.equal(calls.filter((x) => x.cmd === "cmd_save_config").length, 1);
});
let form;
function SettingsProbe() {
  form = useSettingsForm();
  return null;
}
test("changed setting is not lost when leaving before debounce ends", async (t) => {
  reset();
  t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
  const r = await mount(SettingsProbe);
  await act(async () => {
    form.updateForm({ whistlePort: 19999 });
  });
  await unmount(r);
  await act(async () => {
    t.mock.timers.tick(1000);
    await settle();
  });
  assert.equal(calls.filter((x) => x.cmd === "cmd_save_config").length, 1);
});
test("changed external password updates auth proxy", async (t) => {
  reset("external");
  t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
  const r = await mount(SettingsProbe);
  try {
    await act(async () => {
      form.updateForm({ externalPassword: "fixture" });
    });
    await act(async () => {
      t.mock.timers.tick(1000);
      await settle();
    });
    assert.equal(calls.filter((x) => x.cmd === "cmd_start_auth_proxy").length, 1);
  } finally {
    await unmount(r);
  }
});

test("failed proxy switch rejects without reporting success", async () => {
  reset();
  failCommand = "cmd_set_proxy_mode";
  await assert.rejects(useAppStore.getState().setProxyMode("global"), /denied/);
  assert.equal(useAppStore.getState().proxyStatus, null);
});
test("wizard cannot complete when external connection fails", async () => {
  reset();
  let completed = 0;
  const { SetupWizard } = require(resolve(".tooling/tests/frontend.cjs"));
  const r = await mount(() => React.createElement(SetupWizard, { onComplete: () => completed++ }));
  const text = (n) => (typeof n === "string" ? n : (n.children || []).map(text).join(""));
  const click = async (label) =>
    act(async () => {
      await r.root
        .findAllByType("button")
        .find((n) => text(n).includes(label))
        .props.onClick();
      await settle();
    });
  try {
    await click("外部模式");
    await click("下一步");
    failCommand = "cmd_start_whistle";
    await click("完成设置");
    assert.equal(completed, 0);
    assert.equal(
      calls.some((c) => c.cmd === "cmd_save_config" && c.args.config.setup_completed),
      false,
    );
    failCommand = null;
    await click("完成设置");
    assert.equal(completed, 1);
  } finally {
    await unmount(r);
  }
});
