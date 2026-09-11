// Route guard for spec 002 US1: routes back to the onboarding wizard
// while the current device has not yet completed it. The wizard
// persists the alias only at final commit, so a null alias is an
// authoritative "still onboarding" marker — see spec 002 §FR-004.
//
// The onboarding route itself must not attach this middleware (or a
// null-alias vault would trap in a redirect loop). The invariant is
// enforced by omission: only `/workspace`, `/settings`, and `/chat`
// call it.

import { useDevice } from '~/composables/useDevice'

export default defineNuxtRouteMiddleware(async (to) => {
  const params = to.params
  const rawInstance = params.instance
  const instance = typeof rawInstance === 'string'
    ? rawInstance
    : Array.isArray(rawInstance) ? (rawInstance[0] ?? '') : ''
  if (!instance) {
    return
  }
  try {
    const { currentDeviceInfoAsync } = useDevice()
    const info = await currentDeviceInfoAsync()
    if (info.alias === null) {
      return navigateTo(`/onboarding/${instance}`)
    }
  }
  catch {
    // No active instance yet, or backend failure — fall through and
    // let the page's own error handling deal with it. The
    // /chat/[instance] page already surfaces a NoActiveInstance
    // error, and the workspace page never renders anything sensitive
    // until it has device info.
  }
})
