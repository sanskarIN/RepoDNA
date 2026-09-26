import { Component, type ErrorInfo, type ReactNode } from "react";
import { ErrorBox } from "./common";

interface State {
  error: Error | null;
}

/**
 * Keeps one broken view (for example, an artifact missing data the view expects) from
 * taking down the whole interface. Changing `resetKey` clears the error.
 */
export class ErrorBoundary extends Component<{ children: ReactNode; resetKey: string }, State> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidUpdate(previous: { resetKey: string }): void {
    if (previous.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    // Shown to the user below; the console keeps the stack for bug reports.
    console.error(error, info.componentStack);
  }

  override render(): ReactNode {
    if (this.state.error) {
      return (
        <ErrorBox>
          <p>
            <strong>This view could not be shown.</strong> {this.state.error.message}
          </p>
          <p>
            Other views may still work. If this analysis was made by a different RepoDNA version,
            analyzing the repository again usually helps. Please report the problem at
            github.com/sanskarIN/RepoDNA/issues with the steps that led to it.
          </p>
        </ErrorBox>
      );
    }
    return this.props.children;
  }
}
