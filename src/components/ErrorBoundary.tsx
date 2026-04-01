import { relaunch } from "@tauri-apps/plugin-process";
import React from "react";

interface State {
  hasError: boolean;
}

class ErrorBoundary extends React.Component<React.PropsWithChildren, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("[ErrorBoundary] uncaught render error:", error, info);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex min-h-screen items-center justify-center bg-white p-6 dark:bg-gray-950">
          <div className="text-center">
            <p className="text-sm text-gray-700 dark:text-gray-300">
              Something went wrong. Please relaunch FocusRetro.
            </p>
            <button
              type="button"
              onClick={relaunch}
              className="mt-3 cursor-pointer rounded-lg bg-brand-600 px-4 py-1.5 text-xs font-medium text-white hover:bg-brand-500"
            >
              Relaunch
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

export default ErrorBoundary;
