import type { ReactNode } from 'react';
import { Component } from 'react';
import * as IPC from '../../types/ipc';
import { ErrorDialog } from './ErrorDialog';

type Props = {
  children: ReactNode;
};

type State = {
  error: Error | null;
};

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render() {
    if (!this.state.error) return this.props.children;

    return (
      <ErrorDialog
        title="Application error"
        error={{
          code: IPC.ErrorCodes.INTERNAL_ERROR,
          message: 'An unexpected error occurred. Reload the app and try again.',
        }}
        primaryAction={{
          label: 'Reload app',
          onClick: () => window.location.reload(),
        }}
        secondaryAction={{
          label: 'Dismiss',
          onClick: () => this.setState({ error: null }),
        }}
      />
    );
  }
}

