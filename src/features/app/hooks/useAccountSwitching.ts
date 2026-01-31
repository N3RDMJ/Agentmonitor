import { useCallback, useMemo, useRef, useState } from "react";
import { cancelGeminiLogin, runGeminiLogin } from "../../../services/tauri";
import type { AccountSnapshot } from "../../../types";

type UseAccountSwitchingArgs = {
  activeWorkspaceId: string | null;
  accountByWorkspace: Record<string, AccountSnapshot | null | undefined>;
  refreshAccountInfo: (workspaceId: string) => Promise<void> | void;
  refreshAccountRateLimits: (workspaceId: string) => Promise<void> | void;
  alertError: (error: unknown) => void;
};

type UseAccountSwitchingResult = {
  activeAccount: AccountSnapshot | null;
  accountSwitching: boolean;
  handleSwitchAccount: () => Promise<void>;
  handleCancelSwitchAccount: () => Promise<void>;
};

export function useAccountSwitching({
  activeWorkspaceId,
  accountByWorkspace,
  refreshAccountInfo,
  refreshAccountRateLimits,
  alertError,
}: UseAccountSwitchingArgs): UseAccountSwitchingResult {
  const [accountSwitching, setAccountSwitching] = useState(false);
  const accountSwitchCanceledRef = useRef(false);

  const activeAccount = useMemo(() => {
    if (!activeWorkspaceId) {
      return null;
    }
    return accountByWorkspace[activeWorkspaceId] ?? null;
  }, [activeWorkspaceId, accountByWorkspace]);

  const isGeminiLoginCanceled = useCallback((error: unknown) => {
    const message =
      typeof error === "string" ? error : error instanceof Error ? error.message : "";
    const normalized = message.toLowerCase();
    return (
      normalized.includes("gemini login canceled") ||
      normalized.includes("gemini login cancelled") ||
      normalized.includes("request canceled")
    );
  }, []);

  const handleSwitchAccount = useCallback(async () => {
    if (!activeWorkspaceId || accountSwitching) {
      return;
    }
    accountSwitchCanceledRef.current = false;
    setAccountSwitching(true);
    try {
      await runGeminiLogin(activeWorkspaceId);
      if (accountSwitchCanceledRef.current) {
        return;
      }
      await refreshAccountInfo(activeWorkspaceId);
      await refreshAccountRateLimits(activeWorkspaceId);
    } catch (error) {
      if (accountSwitchCanceledRef.current || isGeminiLoginCanceled(error)) {
        return;
      }
      alertError(error);
    } finally {
      setAccountSwitching(false);
      accountSwitchCanceledRef.current = false;
    }
  }, [
    activeWorkspaceId,
    accountSwitching,
    refreshAccountInfo,
    refreshAccountRateLimits,
    alertError,
    isGeminiLoginCanceled,
  ]);

  const handleCancelSwitchAccount = useCallback(async () => {
    if (!activeWorkspaceId || !accountSwitching) {
      return;
    }
    accountSwitchCanceledRef.current = true;
    try {
      await cancelGeminiLogin(activeWorkspaceId);
    } catch (error) {
      alertError(error);
    } finally {
      setAccountSwitching(false);
    }
  }, [activeWorkspaceId, accountSwitching, alertError]);

  return {
    activeAccount,
    accountSwitching,
    handleSwitchAccount,
    handleCancelSwitchAccount,
  };
}
