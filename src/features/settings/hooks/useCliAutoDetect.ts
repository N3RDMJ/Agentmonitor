import { useEffect, useRef, useState } from "react";
import type { AppSettings, CliType, DetectedClis } from "../../../types";
import { detectInstalledClis } from "../../../services/tauri";

const CLI_PRIORITY: CliType[] = ["claude", "codex", "gemini", "cursor"];

function pickBestCli(detected: DetectedClis): CliType | null {
  for (const cli of CLI_PRIORITY) {
    if (detected[cli]) {
      return cli;
    }
  }
  return null;
}

export function useCliAutoDetect(
  settings: AppSettings,
  isLoading: boolean,
  saveSettings: (next: AppSettings) => Promise<AppSettings>,
): DetectedClis | null {
  const [detected, setDetected] = useState<DetectedClis | null>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  useEffect(() => {
    if (isLoading) {
      return;
    }
    let active = true;
    void (async () => {
      try {
        const result = await detectInstalledClis();
        if (!active) {
          return;
        }
        setDetected(result);
        const current = settingsRef.current;
        if (current.cliTypeManuallySet) {
          return;
        }
        const best = pickBestCli(result);
        if (best && best !== current.cliType) {
          await saveSettings({ ...current, cliType: best });
        }
      } catch {
        // Detection is best-effort; ignore failures.
      }
    })();
    return () => {
      active = false;
    };
    // Run once after settings load — intentionally omitting settings/saveSettings
    // from deps to avoid re-running on every settings change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isLoading]);

  return detected;
}
