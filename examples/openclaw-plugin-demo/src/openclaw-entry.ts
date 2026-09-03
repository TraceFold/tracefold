// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * The OpenClaw-installable entry point.
 *
 * This is the one file in this demo that imports `openclaw/plugin-sdk`, so it can only be loaded
 * *inside* a real `openclaw` process. Everything it does is delegate to `plugin.ts`'s `register()`
 * -- the exact function `demo.ts`/`harness.ts` already exercised against a real gx engine (see
 * `req/1034`). This file adds only the wiring OpenClaw's own plugin-sdk requires:
 * `definePluginEntry({ id, name, register(api) { ... } })`, in the shape
 * `extensions/onepassword/index.ts:125` uses (`req/1031` §3 -- read-only observation, no line of
 * OpenClaw source reproduced here or anywhere in this demo).
 *
 * UNTESTED until this file is actually installed and loaded by a real `openclaw` binary -- that is
 * what this lane (the req after `1034`) measures for the first time.
 */
import { definePluginEntry } from "openclaw/plugin-sdk/plugin-entry";
import { GxCliMembrane } from "./gx-cli-membrane.ts";
import { MEDIATED_TOOLS, register } from "./plugin.ts";

// Same shape demo.ts uses (req/1034 §4-1's measured working combination: bed on ext4, target file
// may be on DrvFs). Overridable via env so this file does not hardcode a path that only exists on
// the machine that wrote it.
const projectDir = process.env["GX_ESCROW_PROJECT_DIR"] ?? "/tmp/gx-openclaw-demo-bed";
const homeDir = process.env["GX_ESCROW_HOME_DIR"] ?? "/tmp/gx-openclaw-demo-bed/home";
const actorKeyId = process.env["GX_ESCROW_ACTOR_KEY"] ?? "demo-actor";
const gxBinary = process.env["GX_ESCROW_BINARY"] ?? "gx";

const membrane = new GxCliMembrane({
  command: process.env["GX_ESCROW_COMMAND"] ?? "gx",
  args: [],
  binary: gxBinary,
  projectDir,
  homeDir,
  actorKeyId,
});

const escrowed: { toolCallId: string | null; locator: string; transformationId: string }[] = [];

export default definePluginEntry({
  id: "gx-escrow",
  name: "gx escrow",
  register(api: any) {
    register(api, {
      membrane,
      tools: MEDIATED_TOOLS,
      actorModel: "openclaw-agent",
      escrowed,
      log: (line: string) => {
        try {
          api.logger?.info?.(line);
        } catch {
          // fall through to console below
        }
        console.log(`[gx-escrow] ${line}`);
      },
    });
    console.log("[gx-escrow] register(api) called -- before_tool_call handler registered");
  },
});
