import type { Update } from "@tauri-apps/plugin-updater";

export type MidwayUpdateChannel = "stable" | "beta";

export type AppUpdateInfo = {
  currentVersion: string;
  supported: boolean;
  channel: MidwayUpdateChannel;
  update: Update | null;
  error?: string | null;
};

export type AppUpdateProgress = {
  phase: "started" | "progress" | "finished";
  downloadedBytes: number;
  totalBytes: number | null;
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    __TAURI__?: unknown;
  }
}

function normalizeUpdaterError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }

  return "No pude completar la operación de update.";
}

export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  return Boolean(window.__TAURI_INTERNALS__ || window.__TAURI__);
}

export function resolveUpdateChannel(): MidwayUpdateChannel {
  return import.meta.env.VITE_MIDWAY_UPDATE_CHANNEL === "beta" ? "beta" : "stable";
}

export async function getCurrentAppVersionSafe(): Promise<string> {
  if (!isTauriRuntime()) {
    return import.meta.env.VITE_APP_VERSION ?? "dev";
  }

  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    return await getVersion();
  } catch {
    return import.meta.env.VITE_APP_VERSION ?? "dev";
  }
}

export async function checkForAppUpdate(): Promise<AppUpdateInfo> {
  const currentVersion = await getCurrentAppVersionSafe();
  const channel = resolveUpdateChannel();

  if (!isTauriRuntime() || import.meta.env.DEV) {
    return {
      currentVersion,
      supported: false,
      channel,
      update: null,
      error: "El auto-update solo funciona dentro de la app desktop empaquetada."
    };
  }

  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check({ timeout: 30_000 });

    return {
      currentVersion,
      supported: true,
      channel,
      update,
      error: null
    };
  } catch (error) {
    return {
      currentVersion,
      supported: true,
      channel,
      update: null,
      error: normalizeUpdaterError(error)
    };
  }
}

export async function downloadAndInstallAppUpdate(
  update: Update,
  onProgress?: (progress: AppUpdateProgress) => void
): Promise<void> {
  let downloadedBytes = 0;
  let totalBytes: number | null = null;

  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        totalBytes = event.data.contentLength ?? null;
        downloadedBytes = 0;
        onProgress?.({
          phase: "started",
          downloadedBytes,
          totalBytes
        });
        break;
      case "Progress":
        downloadedBytes += event.data.chunkLength;
        onProgress?.({
          phase: "progress",
          downloadedBytes,
          totalBytes
        });
        break;
      case "Finished":
        onProgress?.({
          phase: "finished",
          downloadedBytes,
          totalBytes
        });
        break;
      default:
        break;
    }
  });
}

export async function relaunchAppSafely(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
