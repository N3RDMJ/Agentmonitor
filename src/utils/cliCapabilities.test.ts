import { describe, expect, it } from "vitest";
import { getCliCapabilities, getCliDisplayName } from "./cliCapabilities";

describe("cliCapabilities", () => {
  it("returns full capabilities for codex", () => {
    expect(getCliCapabilities("codex")).toEqual({
      tier: "full",
      supportsBidirectionalRpc: true,
      supportsInterrupt: true,
      supportsApprovals: true,
      supportsCollaborationModes: true,
      supportsApps: true,
      supportsMcpServers: true,
      supportsCodexConfigSync: true,
    });
  });

  it("returns compatible capabilities for non-codex CLIs", () => {
    for (const cliType of ["gemini", "cursor", "claude"] as const) {
      expect(getCliCapabilities(cliType)).toEqual({
        tier: "compatible",
        supportsBidirectionalRpc: false,
        supportsInterrupt: false,
        supportsApprovals: false,
        supportsCollaborationModes: false,
        supportsApps: false,
        supportsMcpServers: false,
        supportsCodexConfigSync: false,
      });
    }
  });

  it("returns a user-facing display name", () => {
    expect(getCliDisplayName("codex")).toBe("Agent CLI");
    expect(getCliDisplayName("gemini")).toBe("Gemini CLI");
    expect(getCliDisplayName("cursor")).toBe("Cursor CLI");
    expect(getCliDisplayName("claude")).toBe("Claude Code");
  });
});
