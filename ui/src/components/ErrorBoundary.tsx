import { Component, type ReactNode } from "react";

interface State {
  error: Error | null;
}

/** Shows what went wrong instead of an empty window, with a way out that doesn't need a restart. */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error) {
    console.error(error);
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div role="alert" className="crash">
        <h1>Something went wrong</h1>
        <p>Corvus hit an error it couldn't recover from. Reloading usually fixes it.</p>
        <pre>{error.message}</pre>
        <button className="btn btn-primary" onClick={() => window.location.reload()}>
          Reload
        </button>
      </div>
    );
  }
}
