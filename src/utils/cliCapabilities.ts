import type { CliCapabilities, CliType } from "../types";

const FULL_CAPABILITIES: CliCapabilities = {
  tier: "full",
  supportsBidirectionalRpc: true,
  supportsInterrupt: true,
  supportsApprovals: true,
  supportsCollaborationModes: true,
  supportsApps: true,
  supportsMcpServers: true,
  supportsCodexConfigSync: true,
};

const COMPATIBLE_CAPABILITIES: CliCapabilities = {
  tier: "compatible",
  supportsBidirectionalRpc: false,
  supportsInterrupt: false,
  supportsApprovals: false,
  supportsCollaborationModes: false,
  supportsApps: false,
  supportsMcpServers: false,
  supportsCodexConfigSync: false,
};

export function getCliCapabilities(cliType: CliType): CliCapabilities {
  return cliType === "codex" ? FULL_CAPABILITIES : COMPATIBLE_CAPABILITIES;
}

export function getCliDisplayName(cliType: CliType): string {
  switch (cliType) {
    case "gemini":
      return "Gemini CLI";
    case "cursor":
      return "Cursor CLI";
    case "claude":
      return "Claude Code";
    default:
      return "Agent CLI";
  }
}
