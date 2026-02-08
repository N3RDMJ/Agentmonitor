import { useCallback } from "react";
import type { Dispatch, MutableRefObject } from "react";
import type { ApprovalRequest } from "../../../types";
import {
  getApprovalCommandInfo,
  matchesCommandPrefix,
} from "../../../utils/approvalRules";
import { respondToServerRequest } from "../../../services/tauri";
import type { ThreadAction } from "./useThreadsReducer";

type UseThreadApprovalEventsOptions = {
  dispatch: Dispatch<ThreadAction>;
  approvalAllowlistRef: MutableRefObject<Record<string, string[][]>>;
  approvalsEnabled: boolean;
};

export function useThreadApprovalEvents({
  dispatch,
  approvalAllowlistRef,
  approvalsEnabled,
}: UseThreadApprovalEventsOptions) {
  return useCallback(
    (approval: ApprovalRequest) => {
      if (!approvalsEnabled) {
        void respondToServerRequest(
          approval.workspace_id,
          approval.request_id,
          "decline",
        );
        return;
      }
      const commandInfo = getApprovalCommandInfo(approval.params ?? {});
      const allowlist =
        approvalAllowlistRef.current[approval.workspace_id] ?? [];
      if (commandInfo && matchesCommandPrefix(commandInfo.tokens, allowlist)) {
        void respondToServerRequest(
          approval.workspace_id,
          approval.request_id,
          "accept",
        );
        return;
      }
      dispatch({ type: "addApproval", approval });
    },
    [approvalAllowlistRef, approvalsEnabled, dispatch],
  );
}
