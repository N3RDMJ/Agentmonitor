import { useThemePreference } from "../../layout/hooks/useThemePreference";
import { useTransparencyPreference } from "../../layout/hooks/useTransparencyPreference";
import { useUiScaleShortcuts } from "../../layout/hooks/useUiScaleShortcuts";
import { useAppSettings } from "../../settings/hooks/useAppSettings";
import { useCliAutoDetect } from "../../settings/hooks/useCliAutoDetect";

export function useAppSettingsController() {
  const {
    settings: appSettings,
    setSettings: setAppSettings,
    saveSettings,
    doctor,
    isLoading: appSettingsLoading,
  } = useAppSettings();

  const detectedClis = useCliAutoDetect(appSettings, appSettingsLoading, saveSettings);

  useThemePreference(appSettings.theme);
  const { reduceTransparency, setReduceTransparency } =
    useTransparencyPreference();

  const {
    uiScale,
    scaleShortcutTitle,
    scaleShortcutText,
    queueSaveSettings,
  } = useUiScaleShortcuts({
    settings: appSettings,
    setSettings: setAppSettings,
    saveSettings,
  });

  return {
    appSettings,
    setAppSettings,
    saveSettings,
    queueSaveSettings,
    doctor,
    appSettingsLoading,
    detectedClis,
    reduceTransparency,
    setReduceTransparency,
    uiScale,
    scaleShortcutTitle,
    scaleShortcutText,
  };
}
