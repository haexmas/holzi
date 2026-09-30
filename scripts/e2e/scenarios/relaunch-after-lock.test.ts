import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, openChat } from '../lib/flows.ts'
import {
  PROCESS_END_LIMIT_MS,
  RELAUNCH_LIMIT_MS,
} from '../lib/close-promises.ts'
import { isPainted, readFramebuffer } from '../lib/framebuffer.ts'

// Spec 013, FR-018: a relaunching build ends the original process and starts a fresh one over the same
// data, and the new window shows the unlock screen again. The driver session cannot follow the
// relaunch (spike; research R11), so everything past the press is checked from outside: the pid, the
// marker on the new process, the virtual screen's own framebuffer, and finally a second driver session
// over the same data, which is what the relaunched window itself shows.
scenario(
  'relaunch-after-lock',
  { needs: { closeBehavior: 'relaunch' } },
  async (ctx) => {
    const framebufferDir = mkdtempSync(join(tmpdir(), 'e2e-relaunch-fb-'))
    ctx.onTeardown(() =>
      rmSync(framebufferDir, { recursive: true, force: true }),
    )

    const instance = await ctx.startInstance({ framebufferDir })
    await createAndUnlock(instance, { name: 'e2e-test' })
    await openChat(instance)

    const originalPid = instance.pid
    await instance.press('lock-instance-sidebar')

    // (1) the original process ends.
    await instance.waitForEnd(PROCESS_END_LIMIT_MS)

    // (2) a new marked process of the same binary appears - the marker is inherited across the relaunch.
    const relaunched = await ctx.waitFor(
      'a new marked process of the same binary to appear after the relaunch',
      () =>
        instance
          .markedProcesses()
          .find((process) => process.pid !== originalPid),
      { fixed: true, timeoutMs: RELAUNCH_LIMIT_MS },
    )
    assert.ok(
      relaunched !== undefined,
      'the application did not relaunch: no new marked process of the same binary appeared',
    )
    ctx.step('relaunch-seen', `pid ${relaunched.pid}`)

    // (3) that process has painted a window (T068 confirmed the framebuffer route works for this build).
    await ctx.waitFor(
      'the relaunched window to paint the screen',
      () => {
        const buffer = readFileSync(join(framebufferDir, 'Xvfb_screen0'))
        return isPainted(readFramebuffer(buffer))
      },
      { fixed: true, timeoutMs: RELAUNCH_LIMIT_MS },
    )
    ctx.step('relaunch-painted')

    // (4) stop the relaunched process (it cannot be driven; the session died with the original process)
    // and start a fresh instance over the same data - what the relaunched window itself shows: the
    // instance list, not unlocked.
    await instance.stop()
    const fresh = await ctx.startInstance({ reusesRoot: instance.root })
    const entryHook = `[data-testid="instance-entry"][data-instance-name="e2e-test"]`
    // The process being up (ctx.startInstance's own poll) does not mean the page has rendered the
    // instance list yet - poll for it the same way click()/type() would, instead of one bare exec.
    await ctx.waitFor(
      'the created instance to be listed after the relaunch',
      () =>
        fresh.exec<boolean>(
          `return document.querySelector('${entryHook}') !== null`,
        ),
    )
    const path = await fresh.exec<string>('return location.pathname')
    assert.equal(path, '/', `expected the unlock screen's address, got ${path}`)
    ctx.step('relaunch-checked')
  },
)
