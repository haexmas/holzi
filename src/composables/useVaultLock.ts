/**
 * Locks the vault (spec 013, FR-002): flushes the window manager layout first (spec 015 FR-027),
 * then asks the backend to close the instance. The backend replaces the page with a spinner and ends
 * the process, so nothing is navigated or cleared here and a failed call has nothing to show.
 */
export function useVaultLock() {
  const wm = useWindowManagerStore()
  const { closeAsync } = useInstance()

  async function lock() {
    await wm.flushAsync()
    await closeAsync().catch(() => {})
  }

  return { lock }
}
