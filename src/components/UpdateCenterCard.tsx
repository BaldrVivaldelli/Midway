import { useCallback, useEffect, useMemo, useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";
import {
  checkForAppUpdate,
  downloadAndInstallAppUpdate,
  getCurrentAppVersionSafe,
  relaunchAppSafely,
  resolveUpdateChannel,
  type AppUpdateProgress
} from "../lib/updater";

type UpdateCenterCardProps = {
  autoCheck?: boolean;
  className?: string;
};

function formatRelativeTimestamp(value: string | null): string {
  if (!value) {
    return "todavía no chequeado";
  }

  try {
    const date = new Date(value);
    return date.toLocaleString();
  } catch {
    return value;
  }
}

function formatBytes(value: number | null): string {
  if (value === null || value <= 0) {
    return "0 B";
  }

  if (value < 1024) {
    return `${value} B`;
  }

  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }

  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

export default function UpdateCenterCard({
  autoCheck = true,
  className
}: UpdateCenterCardProps) {
  const [currentVersion, setCurrentVersion] = useState<string>(import.meta.env.VITE_APP_VERSION ?? "dev");
  const [channel] = useState(resolveUpdateChannel());
  const [update, setUpdate] = useState<Update | null>(null);
  const [checkedAt, setCheckedAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [supported, setSupported] = useState(true);
  const [busy, setBusy] = useState<"checking" | "installing" | "relaunching" | null>(null);
  const [progress, setProgress] = useState<AppUpdateProgress | null>(null);
  const [installReady, setInstallReady] = useState(false);
  const shouldAutoCheck = autoCheck && !import.meta.env.DEV;

  const checkNow = useCallback(async () => {
    setBusy("checking");
    setError(null);

    const result = await checkForAppUpdate();
    setCurrentVersion(result.currentVersion);
    setSupported(result.supported);
    setUpdate(result.update);
    setError(result.error ?? null);
    setCheckedAt(new Date().toISOString());
    setInstallReady(false);
    setProgress(null);
    setBusy(null);
  }, []);

  useEffect(() => {
    void getCurrentAppVersionSafe().then(setCurrentVersion);

    if (!shouldAutoCheck) {
      return;
    }

    void checkNow();
  }, [shouldAutoCheck, checkNow]);

  const handleInstall = useCallback(async () => {
    if (!update) {
      return;
    }

    setBusy("installing");
    setError(null);
    setProgress(null);

    try {
      await downloadAndInstallAppUpdate(update, (nextProgress) => {
        setProgress(nextProgress);
      });
      setInstallReady(true);
      setUpdate(null);
    } catch (installError) {
      setError(
        installError instanceof Error ? installError.message : "No pude instalar el update."
      );
    } finally {
      setBusy(null);
    }
  }, [update]);

  const handleRelaunch = useCallback(async () => {
    setBusy("relaunching");
    try {
      await relaunchAppSafely();
    } catch (relaunchError) {
      setError(
        relaunchError instanceof Error ? relaunchError.message : "No pude reiniciar la app."
      );
      setBusy(null);
    }
  }, []);

  const progressLabel = useMemo(() => {
    if (!progress) {
      return null;
    }

    if (progress.phase === "started") {
      return `Preparando descarga ${formatBytes(progress.totalBytes)}.`;
    }

    if (progress.phase === "finished") {
      return "Descarga completa. Instalando update…";
    }

    return `${formatBytes(progress.downloadedBytes)} / ${formatBytes(progress.totalBytes)}`;
  }, [progress]);

  const progressRatio = useMemo(() => {
    if (!progress || progress.totalBytes === null || progress.totalBytes <= 0) {
      return 0;
    }

    return Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100));
  }, [progress]);

  return (
    <section className={className ? `card update-center-card ${className}` : "card update-center-card"}>
      <div className="header-row">
        <div>
          <h3 className="section-title">App updates</h3>
          <div className="muted small">
            Versión {currentVersion} · canal {channel}
          </div>
        </div>
        <div className="header-actions">
          <button
            className="button secondary compact-button"
            disabled={busy === "checking" || busy === "installing" || busy === "relaunching"}
            onClick={() => void checkNow()}
            type="button"
          >
            {busy === "checking" ? "Chequeando…" : "Check updates"}
          </button>
        </div>
      </div>

      <div className="stack small-gap">
        <div className="muted small">Último chequeo: {formatRelativeTimestamp(checkedAt)}</div>

        {!supported ? (
          <div className="empty-surface muted small">
            El updater funciona solo en la app desktop empaquetada con Tauri.
          </div>
        ) : null}

        {error ? <div className="inline-alert error">{error}</div> : null}

        {update ? (
          <div className="update-available-card">
            <div className="header-row">
              <div>
                <div className="section-title">Update disponible</div>
                <div className="muted small">
                  {currentVersion} → {update.version}
                  {update.date ? ` · ${new Date(update.date).toLocaleDateString()}` : ""}
                </div>
              </div>
              <span className="status-pill ok">Nueva versión</span>
            </div>

            {update.body ? <pre className="release-notes-block">{update.body}</pre> : null}

            {progress ? (
              <div className="stack small-gap">
                <div className="progress-track" aria-hidden="true">
                  <div className="progress-fill" style={{ width: `${progressRatio}%` }} />
                </div>
                <div className="muted small">{progressLabel}</div>
              </div>
            ) : null}

            <div className="header-actions align-right">
              <button
                className="button compact-button"
                disabled={busy === "installing" || busy === "relaunching"}
                onClick={() => void handleInstall()}
                type="button"
              >
                {busy === "installing" ? "Instalando…" : "Download & install"}
              </button>
            </div>
          </div>
        ) : supported && !error && checkedAt ? (
          <div className="empty-surface muted small">
            No encontré updates disponibles para esta instalación.
          </div>
        ) : null}

        {installReady ? (
          <div className="update-installed-card">
            <div>
              <div className="section-title">Update listo</div>
              <div className="muted small">
                La nueva versión ya está instalada. Reiniciá Midway para aplicar el cambio.
              </div>
            </div>
            <div className="header-actions align-right">
              <button
                className="button compact-button"
                disabled={busy === "relaunching"}
                onClick={() => void handleRelaunch()}
                type="button"
              >
                {busy === "relaunching" ? "Reiniciando…" : "Relaunch"}
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}
