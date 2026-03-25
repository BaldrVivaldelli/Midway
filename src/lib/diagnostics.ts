export type CrashRecord = {
  id: string;
  source: "window.error" | "unhandledrejection" | "react-boundary";
  message: string;
  stack?: string | null;
  createdAt: string;
};

const CRASH_STORAGE_KEY = "midway.diagnostics.crashes.v1";
const MAX_CRASH_RECORDS = 20;

function createId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return Math.random().toString(36).slice(2);
}

export function readCrashRecords(): CrashRecord[] {
  if (typeof window === "undefined") {
    return [];
  }

  try {
    const raw = window.localStorage.getItem(CRASH_STORAGE_KEY);
    if (!raw) {
      return [];
    }

    const parsed = JSON.parse(raw) as CrashRecord[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export function appendCrashRecord(
  input: Omit<CrashRecord, "id" | "createdAt">
): CrashRecord {
  const record: CrashRecord = {
    id: createId(),
    createdAt: new Date().toISOString(),
    ...input
  };

  if (typeof window === "undefined") {
    return record;
  }

  const next = [record, ...readCrashRecords()].slice(0, MAX_CRASH_RECORDS);
  try {
    window.localStorage.setItem(CRASH_STORAGE_KEY, JSON.stringify(next));
  } catch {
    // ignore storage failures
  }

  return record;
}

export function clearCrashRecords(): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.removeItem(CRASH_STORAGE_KEY);
  } catch {
    // ignore storage failures
  }
}

export function installCrashCapture(): () => void {
  if (typeof window === "undefined") {
    return () => undefined;
  }

  const handleError = (event: ErrorEvent) => {
    appendCrashRecord({
      source: "window.error",
      message: event.message || "Unhandled window error",
      stack: event.error instanceof Error ? event.error.stack ?? null : null
    });
  };

  const handleRejection = (event: PromiseRejectionEvent) => {
    const reason = event.reason;
    const message =
      reason instanceof Error
        ? reason.message
        : typeof reason === "string"
          ? reason
          : "Unhandled promise rejection";

    appendCrashRecord({
      source: "unhandledrejection",
      message,
      stack: reason instanceof Error ? reason.stack ?? null : null
    });
  };

  window.addEventListener("error", handleError);
  window.addEventListener("unhandledrejection", handleRejection);

  return () => {
    window.removeEventListener("error", handleError);
    window.removeEventListener("unhandledrejection", handleRejection);
  };
}
