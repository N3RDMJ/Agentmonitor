import type { ReactNode } from "react";
import type { AppSettings } from "../../../../types";
import { fileManagerName, openInFileManagerLabel } from "../../../../utils/platformPaths";

type SettingsFeaturesSectionProps = {
  appSettings: AppSettings;
  hasCodexHomeOverrides: boolean;
  openConfigError: string | null;
  onOpenConfig: () => void;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
};

type ToggleControl = {
  id: string;
  title: string;
  subtitle: ReactNode;
  value: boolean;
  onToggle: () => void;
};

type SelectControl = {
  id: string;
  title: string;
  subtitle: ReactNode;
  value: string;
  ariaLabel: string;
  options: Array<{ value: string; label: string }>;
  onChange: (value: string) => void;
};

type InputControl = {
  id: string;
  title: string;
  subtitle: ReactNode;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
};

const renderToggleControl = (control: ToggleControl) => (
  <div className="settings-toggle-row" key={control.id}>
    <div>
      <div className="settings-toggle-title">{control.title}</div>
      <div className="settings-toggle-subtitle">{control.subtitle}</div>
    </div>
    <button
      type="button"
      className={`settings-toggle ${control.value ? "on" : ""}`}
      onClick={control.onToggle}
      aria-pressed={control.value}
    >
      <span className="settings-toggle-knob" />
    </button>
  </div>
);

const renderSelectControl = (control: SelectControl) => (
  <div className="settings-toggle-row" key={control.id}>
    <div>
      <div className="settings-toggle-title">{control.title}</div>
      <div className="settings-toggle-subtitle">{control.subtitle}</div>
    </div>
    <select
      id={control.id}
      className="settings-select"
      value={control.value}
      onChange={(event) => control.onChange(event.target.value)}
      aria-label={control.ariaLabel}
    >
      {control.options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  </div>
);

const renderInputControl = (control: InputControl) => (
  <div className="settings-toggle-row" key={control.id}>
    <div>
      <div className="settings-toggle-title">{control.title}</div>
      <div className="settings-toggle-subtitle">{control.subtitle}</div>
    </div>
    <input
      id={control.id}
      className="settings-input settings-input-inline"
      value={control.value}
      placeholder={control.placeholder}
      onChange={(event) => control.onChange(event.target.value)}
    />
  </div>
);

export function SettingsFeaturesSection({
  appSettings,
  hasCodexHomeOverrides,
  openConfigError,
  onOpenConfig,
  onUpdateAppSettings,
}: SettingsFeaturesSectionProps) {
  const updateSettings = (patch: Partial<AppSettings>) => {
    void onUpdateAppSettings({
      ...appSettings,
      ...patch,
    });
  };

  const stableControls: Array<ToggleControl | SelectControl | InputControl> = [
    {
      id: "features-model-input",
      title: "Model",
      subtitle: (
        <>
          Set top-level <code>model</code> in config.toml.
        </>
      ),
      value: appSettings.codexModel ?? "",
      placeholder: "gpt-5",
      onChange: (value) =>
        updateSettings({
          codexModel: value.trim() ? value : null,
        }),
    },
    {
      id: "features-model-provider-input",
      title: "Model Provider",
      subtitle: (
        <>
          Choose top-level <code>model_provider</code>.
        </>
      ),
      value: appSettings.codexModelProvider ?? "",
      placeholder: "openai",
      onChange: (value) =>
        updateSettings({
          codexModelProvider: value.trim() ? value : null,
        }),
    },
    {
      id: "features-reasoning-effort-select",
      title: "Reasoning Effort",
      subtitle: (
        <>
          Set <code>model_reasoning_effort</code>.
        </>
      ),
      value: appSettings.codexModelReasoningEffort,
      ariaLabel: "Reasoning Effort",
      options: [
        { value: "minimal", label: "Minimal" },
        { value: "low", label: "Low" },
        { value: "medium", label: "Medium" },
        { value: "high", label: "High" },
      ],
      onChange: (value) =>
        updateSettings({
          codexModelReasoningEffort: value as AppSettings["codexModelReasoningEffort"],
        }),
    },
    {
      id: "features-approval-policy-select",
      title: "Approval Policy",
      subtitle: (
        <>
          Set top-level <code>approval_policy</code>.
        </>
      ),
      value: appSettings.codexApprovalPolicy,
      ariaLabel: "Approval Policy",
      options: [
        { value: "untrusted", label: "Untrusted" },
        { value: "on-failure", label: "On failure" },
        { value: "on-request", label: "On request" },
        { value: "never", label: "Never" },
      ],
      onChange: (value) =>
        updateSettings({
          codexApprovalPolicy: value as AppSettings["codexApprovalPolicy"],
        }),
    },
    {
      id: "features-sandbox-mode-select",
      title: "Sandbox Mode",
      subtitle: (
        <>
          Set top-level <code>sandbox_mode</code>.
        </>
      ),
      value: appSettings.codexSandboxMode,
      ariaLabel: "Sandbox Mode",
      options: [
        { value: "read-only", label: "Read only" },
        { value: "workspace-write", label: "Workspace write" },
        { value: "danger-full-access", label: "Danger full access" },
      ],
      onChange: (value) =>
        updateSettings({
          codexSandboxMode: value as AppSettings["codexSandboxMode"],
        }),
    },
    {
      id: "features-web-search-select",
      title: "Web Search",
      subtitle: (
        <>
          Set top-level <code>web_search</code>.
        </>
      ),
      value: appSettings.codexWebSearch,
      ariaLabel: "Web Search",
      options: [
        { value: "live", label: "Live" },
        { value: "cached", label: "Cached" },
      ],
      onChange: (value) =>
        updateSettings({
          codexWebSearch: value as AppSettings["codexWebSearch"],
        }),
    },
    {
      id: "features-auth-credentials-store-select",
      title: "Auth Credentials Store",
      subtitle: (
        <>
          Set <code>cli_auth_credentials_store</code>.
        </>
      ),
      value: appSettings.codexCliAuthCredentialsStore,
      ariaLabel: "Auth Credentials Store",
      options: [
        { value: "auto", label: "Auto" },
        { value: "keyring", label: "Keyring" },
        { value: "file", label: "File" },
      ],
      onChange: (value) =>
        updateSettings({
          codexCliAuthCredentialsStore: value as AppSettings["codexCliAuthCredentialsStore"],
        }),
    },
    {
      id: "features-auth-method-input",
      title: "Preferred Auth Method",
      subtitle: (
        <>
          Set top-level <code>preferred_auth_method</code>.
        </>
      ),
      value: appSettings.codexPreferredAuthMethod ?? "",
      placeholder: "chatgpt",
      onChange: (value) =>
        updateSettings({
          codexPreferredAuthMethod: value.trim() ? value : null,
        }),
    },
    {
      id: "features-check-for-updates-toggle",
      title: "Check for Updates",
      subtitle: (
        <>
          Set top-level <code>check_for_updates</code>.
        </>
      ),
      value: appSettings.codexCheckForUpdates,
      onToggle: () =>
        updateSettings({
          codexCheckForUpdates: !appSettings.codexCheckForUpdates,
        }),
    },
    {
      id: "features-collaboration-modes-toggle",
      title: "Collaboration modes",
      subtitle: "Enable collaboration mode presets (Code, Plan).",
      value: appSettings.collaborationModesEnabled,
      onToggle: () =>
        updateSettings({
          collaborationModesEnabled: !appSettings.collaborationModesEnabled,
        }),
    },
    {
      id: "features-personality-select",
      title: "Personality",
      subtitle: (
        <>
          Choose Codex communication style (writes top-level <code>personality</code> in
          config.toml).
        </>
      ),
      value: appSettings.personality,
      ariaLabel: "Personality",
      options: [
        { value: "friendly", label: "Friendly" },
        { value: "pragmatic", label: "Pragmatic" },
      ],
      onChange: (value) =>
        updateSettings({
          personality: value as AppSettings["personality"],
        }),
    },
    {
      id: "features-steer-toggle",
      title: "Steer mode",
      subtitle: "Send messages immediately. Use Tab to queue while a run is active.",
      value: appSettings.steerEnabled,
      onToggle: () =>
        updateSettings({
          steerEnabled: !appSettings.steerEnabled,
        }),
    },
    {
      id: "features-unified-exec-toggle",
      title: "Background terminal",
      subtitle: "Run long-running terminal commands in the background.",
      value: appSettings.unifiedExecEnabled,
      onToggle: () =>
        updateSettings({
          unifiedExecEnabled: !appSettings.unifiedExecEnabled,
        }),
    },
  ];

  const experimentalControls: ToggleControl[] = [
    {
      id: "features-shell-tool-toggle",
      title: "Shell tool",
      subtitle: (
        <>
          Set <code>[features].shell_tool</code>.
        </>
      ),
      value: appSettings.codexShellToolEnabled,
      onToggle: () =>
        updateSettings({
          codexShellToolEnabled: !appSettings.codexShellToolEnabled,
        }),
    },
    {
      id: "features-shell-snapshot-toggle",
      title: "Shell snapshot",
      subtitle: (
        <>
          Set <code>[features].shell_snapshot</code>.
        </>
      ),
      value: appSettings.codexShellSnapshotEnabled,
      onToggle: () =>
        updateSettings({
          codexShellSnapshotEnabled: !appSettings.codexShellSnapshotEnabled,
        }),
    },
    {
      id: "features-apply-patch-freeform-toggle",
      title: "Apply patch freeform",
      subtitle: (
        <>
          Set <code>[features].apply_patch_freeform</code>.
        </>
      ),
      value: appSettings.codexApplyPatchFreeformEnabled,
      onToggle: () =>
        updateSettings({
          codexApplyPatchFreeformEnabled: !appSettings.codexApplyPatchFreeformEnabled,
        }),
    },
    {
      id: "features-exec-policy-toggle",
      title: "Exec policy",
      subtitle: (
        <>
          Set <code>[features].exec_policy</code>.
        </>
      ),
      value: appSettings.codexExecPolicyEnabled,
      onToggle: () =>
        updateSettings({
          codexExecPolicyEnabled: !appSettings.codexExecPolicyEnabled,
        }),
    },
    {
      id: "features-smart-approvals-toggle",
      title: "Smart approvals",
      subtitle: (
        <>
          Set <code>[features].smart_approvals</code>.
        </>
      ),
      value: appSettings.codexSmartApprovalsEnabled,
      onToggle: () =>
        updateSettings({
          codexSmartApprovalsEnabled: !appSettings.codexSmartApprovalsEnabled,
        }),
    },
    {
      id: "features-remote-compaction-toggle",
      title: "Remote compaction",
      subtitle: (
        <>
          Set <code>[features].remote_compaction</code>.
        </>
      ),
      value: appSettings.codexRemoteCompactionEnabled,
      onToggle: () =>
        updateSettings({
          codexRemoteCompactionEnabled: !appSettings.codexRemoteCompactionEnabled,
        }),
    },
    {
      id: "features-experimental-windows-sandbox-toggle",
      title: "Windows sandbox (experimental)",
      subtitle: (
        <>
          Set <code>[features].experimental_windows_sandbox</code>.
        </>
      ),
      value: appSettings.codexExperimentalWindowsSandboxEnabled,
      onToggle: () =>
        updateSettings({
          codexExperimentalWindowsSandboxEnabled: !appSettings.codexExperimentalWindowsSandboxEnabled,
        }),
    },
    {
      id: "features-elevated-windows-sandbox-toggle",
      title: "Windows sandbox (elevated)",
      subtitle: (
        <>
          Set <code>[features].elevated_windows_sandbox</code>.
        </>
      ),
      value: appSettings.codexElevatedWindowsSandboxEnabled,
      onToggle: () =>
        updateSettings({
          codexElevatedWindowsSandboxEnabled: !appSettings.codexElevatedWindowsSandboxEnabled,
        }),
    },
    {
      id: "features-experimental-collab-toggle",
      title: "Multi-agent",
      subtitle: "Enable multi-agent collaboration tools in Codex.",
      value: appSettings.experimentalCollabEnabled,
      onToggle: () =>
        updateSettings({
          experimentalCollabEnabled: !appSettings.experimentalCollabEnabled,
        }),
    },
    {
      id: "features-experimental-apps-toggle",
      title: "Apps",
      subtitle: (
        <>
          Enable ChatGPT apps/connectors and the <code>/apps</code> command.
        </>
      ),
      value: appSettings.experimentalAppsEnabled,
      onToggle: () =>
        updateSettings({
          experimentalAppsEnabled: !appSettings.experimentalAppsEnabled,
        }),
    },
  ];

  return (
    <section className="settings-section">
      <div className="settings-section-title">Features</div>
      <div className="settings-section-subtitle">
        Manage stable and experimental Codex features.
      </div>
      {hasCodexHomeOverrides && (
        <div className="settings-help">
          Feature settings are stored in the default CODEX_HOME config.toml.
          <br />
          Workspace overrides are not updated.
        </div>
      )}
      <div className="settings-toggle-row">
        <div>
          <div className="settings-toggle-title">Config file</div>
          <div className="settings-toggle-subtitle">
            Open the Codex config in {fileManagerName()}.
          </div>
        </div>
        <button type="button" className="ghost" onClick={onOpenConfig}>
          {openInFileManagerLabel()}
        </button>
      </div>
      {openConfigError && <div className="settings-help">{openConfigError}</div>}
      <div className="settings-subsection-title">Stable Features</div>
      <div className="settings-subsection-subtitle">
        Production-ready features enabled by default.
      </div>
      {stableControls.map((control) => {
        if ("onToggle" in control) {
          return renderToggleControl(control);
        }
        if ("options" in control) {
          return renderSelectControl(control);
        }
        return renderInputControl(control);
      })}
      <div className="settings-subsection-title">Experimental Features</div>
      <div className="settings-subsection-subtitle">
        Preview features that may change or be removed.
      </div>
      {experimentalControls.map(renderToggleControl)}
    </section>
  );
}
