/** Startup branding shared by the web version and the Tauri desktop window.
 * Visible only until the app's local state initializes, never for a forced delay.
 */
export function SplashScreen() {
  return (
    <div className="startup-screen" role="status" aria-live="polite" aria-label="Starting RepoDNA">
      <div className="startup-panel">
        <img className="startup-icon" src="./favicon.svg" width={112} height={112} alt="" />
        <strong className="startup-wordmark">
          Repo<span>DNA</span>
        </strong>
        <p>Understand your codebase. See its DNA.</p>
        <div className="startup-indicator" aria-hidden="true" />
        <span className="startup-status">Opening your local workspace…</span>
      </div>
    </div>
  );
}
