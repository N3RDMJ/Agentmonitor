import { readGlobalGeminiConfigToml, writeGlobalGeminiConfigToml } from "../../../services/tauri";
import { useFileEditor } from "../../shared/hooks/useFileEditor";

export function useGlobalGeminiConfigToml() {
  return useFileEditor({
    key: "global-config",
    read: readGlobalGeminiConfigToml,
    write: writeGlobalGeminiConfigToml,
    readErrorTitle: "Couldn't load global config.toml",
    writeErrorTitle: "Couldn't save global config.toml",
  });
}
