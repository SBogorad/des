import { Component, type ErrorInfo, type ReactNode } from 'react'

type ErrorBoundaryProps = { children: ReactNode }
type ErrorBoundaryState = { error: Error | null }

/** Shows the actual browser error instead of leaving a blank application shell. */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Visualization rendering failed:', error, info)
  }

  render() {
    if (!this.state.error) return this.props.children

    return (
      <main style={styles.page}>
        <section style={styles.card}>
          <div style={styles.eyebrow}>ОШИБКА ВИЗУАЛИЗАЦИИ</div>
          <h1 style={styles.title}>Экран не удалось отрисовать</h1>
          <p style={styles.text}>
            Интерфейс перехватил ошибку вместо пустого чёрного экрана. Обновите страницу один раз.
          </p>
          <pre style={styles.error}>{this.state.error.message}</pre>
          <button style={styles.button} onClick={() => window.location.reload()}>
            Обновить страницу
          </button>
        </section>
      </main>
    )
  }
}

const styles: Record<string, React.CSSProperties> = {
  page: {
    width: '100vw', height: '100vh', display: 'grid', placeItems: 'center',
    background: '#101828', color: '#e5edff', padding: 24,
  },
  card: {
    width: 'min(640px, 100%)', padding: 28, borderRadius: 12,
    border: '1px solid #5a2430', background: '#161d2c',
  },
  eyebrow: { color: '#ff6677', fontSize: 11, letterSpacing: 1.5, fontWeight: 700 },
  title: { margin: '10px 0', fontSize: 22 },
  text: { color: '#a9b8d5', lineHeight: 1.5 },
  error: {
    whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', margin: '16px 0', padding: 12,
    borderRadius: 6, background: '#220f16', color: '#ffb4c0', fontSize: 12,
  },
  button: {
    border: '1px solid #4488ff', borderRadius: 6, padding: '8px 12px',
    background: '#1a3060', color: '#fff', cursor: 'pointer',
  },
}
