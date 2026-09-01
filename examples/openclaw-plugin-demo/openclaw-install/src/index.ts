// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * Installable entry point (manifest form), copy of `../../src/openclaw-entry.ts`'s wiring, kept
 * separate because `openclaw.plugin.json`'s `activation.onStartup: true` is what makes a
 * `plugins.load.paths`-discovered plugin actually load at gateway boot -- a bare `.ts` file with no
 * manifest is discoverable (`plugins list`/`doctor` show it "loaded") but is never scheduled for
 * startup activation. That distinction is itself a measured fact of this lane, not an assumption
 * (`installed-plugin-index-*.js:1198`, `sidecar: record.activation?.onStartup === true`, read-only
 * grep of the installed package -- no line reproduced beyond the field name and comparison already
 * quoted here).
 */
import { definePluginEntry } from "openclaw/plugin-sdk/plugin-entry";
import { GxCliMembrane } from "../../src/gx-cli-membrane.ts";
import { register } from "../../src/plugin.ts";

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
      tools: ["write"],
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
