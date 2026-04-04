import { Component, type ErrorInfo, type ReactNode } from "react";
import { appendCrashRecord } from "../lib/diagnostics";

type Props = {
  children: ReactNode;
};

type State = {
  error: Error | null;
};

export default class AppErrorBoundary extends Component<Props, State> {
  state: State = {
    error: null
  };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    appendCrashRecord({
      source: "react-boundary",
      message: error.message || "React render crash",
      stack: [error.stack, info.componentStack].filter(Boolean).join("\n\n") || null
    });
  }

  handleReload = () => {
    if (typeof window !== "undefined") {
      window.location.reload();
    }
  };

  render(): ReactNode {
    if (!this.state.error) {
      return this.props.children;
    }

    return (
      <div className="fatal-shell">
        <div className="fatal-card">
          <span className="pill subtle">Midway</span>
          <h1>La app se recuperó de un fallo inesperado</h1>
          <p className="muted">
            Guardé el error en diagnósticos para soporte. Podés recargar la app y recuperar la sesión
            autoguardada.
          </p>
          <div className="inline-code-block">{this.state.error.message}</div>
          <div className="row gap wrap">
            <button type="button" className="button primary" onClick={this.handleReload}>
              Recargar la app
            </button>
          </div>
        </div>
      </div>
    );
  }
}
