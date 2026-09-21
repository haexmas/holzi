# Feature Specification: Vault Lifecycle Isolation

**Feature Branch**: `013-vault-lifecycle-isolation`

**Created**: 2026-09-21

**Status**: Draft

**Input**: User description: "Closing a vault must reliably and immediately terminate everything
that belonged to it, and a newly opened vault must never see data from a previously open one. One
running app process hosts at most one vault session; closing a vault ends that process (by default
the app relaunches to the unlock screen). Closing can never be refused and never needs to be
repeated, even for a user who has to leave a shared computer at once. The vault passphrase must not
remain in memory after the vault is closed and must never appear in logs or diagnostic output.
Several independent app processes, each with its own vault (for example work and private), must
keep working side by side without affecting each other."

## Clarifications

### Session 2026-09-21

- Q: Should closing a vault switch to another vault inside the same running app? → A: No. One app
  process serves at most one vault session over its lifetime; switching vaults means closing (the
  process ends) and unlocking again.
- Q: What happens to the app when a vault is closed? → A: By default the app relaunches and shows
  the unlock screen. Exiting the app instead is acceptable; the isolation guarantees are the same.
- Q: How long may closing take? → A: The vault stops answering at once; running work is cancelled
  at once; a bounded grace period of about 1 second (about 3 seconds at most) lets the database close
  cleanly; then the session ends regardless of whether all work has stopped.
- Q: Must two independent app processes on one computer keep working? → A: Yes. Each has its own
  vault, and closing or restarting one has no effect on the other.
- Q: How are model files shared between app processes, and how does a user keep them off a computer
  they do not own? → A: This feature leaves model files as they are today, shared app-wide. Use
  without traces on a foreign computer is the job of a separate, later portable mode in which the
  app keeps all its data in a location the user chooses, for example a USB stick. Normal
  installations get no additional encryption of model files.
- Q: May the unlock form keep the passphrase after a failed attempt? → A: Yes, so the user can
  correct a typo. It is erased when the unlock succeeds, when the form is dismissed and when a close
  begins. Overwriting it with random data would add nothing, because the interface runtime cannot
  overwrite a string in place.
- Q: How do we stop a freshly started app process from deleting another running process's work in
  progress as a "leftover"? → A: Each running app process announces itself through one shared
  marker for as long as it lives. Leftovers of ended or crashed processes (a half-created vault, a
  partial model download or import) are removed only by a process that finds no other app process
  running.
- Q: May the app end its process forcibly when the normal ending does not work? → A: Yes. If the
  app process has not ended about half a second after it asked to end, it is ended forcibly, so it
  is gone within about 4 seconds in every case. A forced end skips the clean shutdown, which is
  accepted: the database recovers on the next open and the operating system releases the memory.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Closing a vault locks everything at once, every time (Priority: P1)

A user works in a vault on a shared computer, for example in an internet cafe. A chat reply is still
streaming and a model download is running when they have to leave immediately. They press close and
walk away. From that moment nothing of the vault may answer any request or show any content, the
running work is cancelled, and within a few seconds the vault session is over without the user doing
anything else. Today closing can even fail with an "operation in progress" error while a reply is
running, and it does not cancel that reply.

**Why this priority**: This is the security promise of the whole feature. A vault that is not
reliably closed when the user says so is not protected, and a close the user has to repeat is not
usable when leaving in a hurry.

**Independent Test**: Start a long chat reply and a model download, press close, and observe. Fully
testable on its own and delivers the core promise: the vault is gone within seconds without further
input.

**Acceptance Scenarios**:

1. **Given** a vault is open and a chat reply is streaming, **When** the user closes the vault,
   **Then** the reply stops at once, no further text is shown or stored after the close request, and
   the vault's content is no longer displayed.
2. **Given** a vault is open with a model download running, **When** the user closes the vault,
   **Then** the download is cancelled and an incomplete download is never presented as installed.
3. **Given** a vault is open and some running work does not react to cancellation, **When** the user
   closes the vault, **Then** after the bounded grace period the vault session ends anyway and the
   user is never asked to close again.
4. **Given** the user has requested a close, **When** any further request of that vault session
   arrives, **Then** it is rejected with a clear "vault closed" outcome instead of a result.
5. **Given** a close is already in progress, **When** the user requests it again or closes the
   window, **Then** the outcome is the same single, consistent ending of the session.

---

### User Story 2 - A new vault never sees anything from the previous one (Priority: P1)

A user closes their work vault and unlocks their private vault. Nothing that belonged to the work
vault may be visible in the private one, in any part of the app: not model lists, chat titles,
drafts, selected models, preferences, pending approvals, error messages, notifications or cached
names. The private vault must start exactly as a first launch of the app would.

**Why this priority**: Equal in weight to Story 1. Leaking one vault's data into another defeats the
purpose of having separate vaults, and today a running app keeps per-vault values in memory across a
vault change.

**Independent Test**: Put recognizable marker values into every kind of per-vault data in vault A,
close it, unlock vault B, and search everything the app shows or holds for those markers. Delivers
the isolation guarantee independently of the other stories.

**Acceptance Scenarios**:

1. **Given** vault A was used and closed, **When** the user unlocks vault B, **Then** none of vault
   A's values (lists, titles, drafts, selections, preferences, decisions, errors, names) appear in
   vault B, in the interface or in the app's working state.
2. **Given** a vault is open, **When** something tries to open or create another vault in the same
   running app, **Then** the attempt is refused with a clear message and the open vault is unaffected.
3. **Given** the user wants to switch vaults, **When** they close the current vault and unlock
   another, **Then** the other vault starts from the same pristine state as a first launch.

---

### User Story 3 - The passphrase never lingers (Priority: P2)

A user's vault passphrase is the key to everything. It may be used to open the vault and nothing
else. After the vault is open, the app no longer needs it, so no copy under the app's control may
remain longer than necessary, and none may exist once the vault session is over. It must never show
up in logs, error messages or diagnostic output, not even when unlocking fails.

**Why this priority**: The passphrase is the most sensitive value in the app. It ranks just below
the P1 stories only because the process ending in Story 1 already bounds most exposure, while this
story shrinks exposure while the vault is open and protects against accidental disclosure.

**Independent Test**: Unlock with a known test passphrase, use, fail once with a wrong passphrase,
close, and search all logs and diagnostic output for the passphrase; check that copies the app
controls are erased once released. Testable independently.

**Acceptance Scenarios**:

1. **Given** the user unlocks a vault with a passphrase, **When** the vault has opened, **Then** the
   app holds no further copy of the passphrase that it controls, beyond what the database itself
   needs while the vault is open.
2. **Given** any log output, error message or diagnostic representation of the app's data, **When**
   it is produced during unlock, use, wrong-passphrase failure and close, **Then** it never contains
   the passphrase.
3. **Given** the user has typed the passphrase into the unlock or create form, **When** the unlock
   succeeds, the form is dismissed or the vault starts closing, **Then** the form no longer holds
   the value, and it was never stored in shared app state or on disk. After a failed attempt the
   form may keep the value so the user can correct a typo.
4. **Given** the user unlocks an already open vault again only to verify the passphrase, **When**
   the check finishes, **Then** no extra database connection or copy remains open.

---

### User Story 4 - Two independent app processes side by side (Priority: P2)

A user keeps their work vault open in one app process and their private vault open in another, on
the same computer. Closing the work vault ends only that process. The private vault, its running
work and its window are untouched.

**Why this priority**: The single-vault-per-process rule would otherwise seem to forbid this
workflow. The feature must make the process boundary the isolation tool without taking away running
two vaults at once.

**Independent Test**: Start two app processes, open a different vault in each, run a chat reply in
one, close the other, and observe. Testable independently.

**Acceptance Scenarios**:

1. **Given** two app processes with different vaults open, **When** the user closes or restarts one,
   **Then** the other keeps its vault open and its running work continues uninterrupted.
2. **Given** a vault is open in one app process, **When** the user tries to open the same vault in
   another app process, **Then** it is refused with a clear, understandable message and no vault
   data is changed.
3. **Given** a vault's app process ended or crashed, **When** the user opens that vault anywhere,
   **Then** it opens normally with no leftover lock.
4. **Given** one app process created a new vault, **When** another app process shows its list of
   vaults, **Then** the new vault is included without restarting anything.
5. **Given** two app processes install the same model at the same time, **When** both finish,
   **Then** exactly one complete, valid installation exists and neither process is left with a
   partial or corrupt file.
6. **Given** two app processes write diagnostic output to a location they share, **When** both run,
   **Then** the output contains no vault content and does not become corrupt.
7. **Given** one app process is creating a vault or downloading a model, **When** another app
   process starts (for example the relaunch after a close), **Then** the work in progress is
   untouched and completes normally.

---

### User Story 5 - Reading state never fails because other work is running (Priority: P3)

A user opens or returns to the chat view while a reply is streaming or a model is loading. The chat
view shows the active model and its settings normally instead of an "operation in progress" error,
and the settings that depend on them (such as the effort control) are correct.

**Why this priority**: This was the visible bug that started the investigation and is already fixed
on this branch. It is kept here so the rule that read-only lookups never block or fail because of
other work is specified and stays covered by tests.

**Independent Test**: Return to the chat view while a reply is streaming and while a model loads,
and trigger two simultaneous lookups of the active model. Testable independently.

**Acceptance Scenarios**:

1. **Given** a chat reply is streaming, **When** the chat view asks which model is active, **Then**
   it gets the answer and shows no error.
2. **Given** two lookups of the active model happen at the same time, **When** both complete,
   **Then** both succeed.

---

### Edge Cases

- The user closes the vault while it is still being unlocked: opening is cancelled, no half-open
  vault remains, and the outcome is a closed vault.
- The user closes the vault while data is being written: everything completed before the close is
  intact, and the vault opens normally on the next unlock.
- Work interrupted by a close leaves nothing that looks complete but is not, such as a partly
  received reply or a partly downloaded model.
- The operating system closes the window or the user quits the app: the same guarantees as an
  explicit close apply.
- The app crashes or is killed: no secret was ever written to disk, and the next launch starts
  normally with no leftover lock.
- An app process crashes while creating a vault or downloading a model: the leftover is never
  offered as a vault or as an installed model, and it is removed by the next app process that
  starts while no other app process is running.
- The user starts another app process for the same vault while the old one is still finishing its
  close: opening waits briefly, at most as long as the close takes, before refusing.
- Relaunching after a close fails: the app exits instead, and the vault is closed either way.
- The normal ending does not work, for example because the window system stops reacting: the app
  process is ended forcibly shortly after, and the vault is closed either way.
- The relaunched app process starts with the configuration of the one that ended, such as a chosen
  data location, and never silently falls back to defaults on the computer.
- A running tool or child process ignores cancellation: it is stopped with the session and cannot
  keep the vault open past the bounded grace period.
- The user presses close many times or closes the window during the grace period: one consistent
  ending, never an error.
- Unsaved drafts in the interface are not preserved across a close: isolation takes precedence.

## Requirements _(mandatory)_

### Functional Requirements

**Closing**

- **FR-001**: From the moment a close is requested, the system MUST NOT start any request belonging
  to the vault session, and no result of the vault session MUST reach the interface. New requests
  are rejected with a clear "vault closed" outcome. Long-running work still in flight is cut and ends
  with the same outcome. A short request that is already executing may finish its work, but its result
  is discarded together with the page.
- **FR-002**: A close request MUST always be accepted. It MUST NOT fail or be refused because work is
  running, and repeating it MUST have no additional effect.
- **FR-003**: On close, the system MUST cancel all running work that belongs to the vault session,
  including chat replies with their tools and sub-agents, model loads and background preloads,
  downloads and imports, processes started on behalf of the vault, and voice capture.
- **FR-004**: After cancelling, the system MUST allow a bounded grace period for a clean shutdown of
  the vault's database: about 1 second for work that stops cooperatively and about 3 seconds in
  total from the close request, after which the session ends regardless.
- **FR-005**: When the session ends, the app process MUST end. By default the app MUST relaunch to
  the unlock screen; exiting instead MUST be an accepted alternative with identical guarantees. The
  user MUST NOT need to close the vault a second time. If the app process has not ended on its own
  about half a second after it asked to end, it MUST be ended forcibly. A relaunched app process
  MUST run with the same configuration as the one that ended, including where it keeps its data, and
  MUST NOT fall back to defaults.
- **FR-006**: The grace deadlines MUST apply only after a close was requested. Slow requests during
  normal use MUST NOT be interrupted by them.
- **FR-007**: The same guarantees MUST apply when the session ends because the window is closed or
  the app is quit.
- **FR-008**: Work interrupted by a close MUST NOT leave data that appears complete but is not.
- **FR-009**: As soon as a close is requested, the interface MUST stop showing the vault's content.

**Isolation**

- **FR-010**: One app process MUST serve at most one vault session over its whole lifetime. Opening
  or creating a vault while a session is active MUST be refused with a clear message and MUST leave
  the active session unchanged. A session starts when a vault has opened successfully; a failed
  unlock or create attempt starts no session, and the unlock screen can be used again in the same
  app process.
- **FR-011**: A vault unlocked after a close MUST run in a fresh app process that starts from the
  same state as a first launch, in the backend and the interface. No per-vault value of the earlier
  vault MAY carry over.
- **FR-012**: The interface MUST NOT keep vault data in any place that outlives the vault session,
  including on-screen state, drafts, notifications and cached names.

**Secrets**

- **FR-013**: The passphrase MUST be used only to open or verify the vault. After opening, nothing
  under the app's control MAY keep it beyond what the database engine needs itself while the vault
  is open.
- **FR-014**: Every copy of the passphrase or a key derived from it that the app's own code holds
  MUST be erased from memory as soon as it is no longer needed. The code MUST NOT make copies it does
  not need.
- **FR-015**: The passphrase MUST NOT appear in any log, error message, diagnostic representation of
  a data structure, or report the app produces, including when unlocking fails.
- **FR-016**: Passphrase entry MUST keep the value only until the unlock succeeds, the form is
  dismissed or a close begins. After a failed attempt it MAY keep the value so the user can correct
  it. It MUST NOT store the value in shared app state or on disk.
- **FR-017**: A passphrase-only verification of an open vault MUST NOT leave an extra connection or
  copy behind once it finishes.

**Several app processes**

- **FR-018**: Any number of app processes MUST be able to run at the same time, each with at most
  one vault. Closing, restarting or crashing one MUST NOT affect the vault, the running work or the
  window of another.
- **FR-019**: Opening a vault that another app process on the same computer has open MUST be refused
  with a clear, localized message and MUST NOT change any vault data.
- **FR-020**: A vault held by an app process that ended or crashed MUST be openable immediately
  afterwards, with no leftover lock.
- **FR-021**: The list of available vaults MUST be current whenever it is shown, including vaults
  created by other app processes since it was last shown.
- **FR-022**: When several app processes install the same model at the same time, exactly one
  complete, valid installation MUST result, with no partial or corrupt files.
- **FR-023**: Diagnostic output written to locations shared between app processes MUST contain no
  vault content and MUST NOT become corrupt through concurrent writers.
- **FR-024**: Model files MUST stay in the location shared by all app processes, as today. This
  feature neither moves nor hides them. Keeping a vault's own model files off a computer the user
  does not own is out of scope and is handled by a separate portable mode.
- **FR-026**: Starting an app process, including the relaunch after a close, MUST NOT delete, alter
  or break work in progress of another running app process, such as a vault being created or a model
  being downloaded or imported. Leftovers of ended or crashed processes MUST be removed only when no
  other app process is running.

**Reads**

- **FR-025**: Read-only lookups, such as which model is active, MUST NOT fail or wait because
  another lookup, a chat reply or a model load is running.

### Key Entities

- **Vault session**: The period in which one vault is open in one app process. It starts at unlock
  and ends at close. It carries all runtime state that belongs to that vault and holds exactly one
  vault.
- **App process**: One running instance of the app on a computer. It serves at most one vault
  session in its lifetime and is independent of every other app process. In the code a vault is
  called an "instance"; this specification uses "app process" for the running app to avoid mixing
  the two.
- **Close request**: The user's or operating system's request to end the vault session. It cannot be
  refused or undone and has a bounded time to completion.
- **Passphrase**: The user's secret that opens the vault and serves as its encryption key. Held only
  transiently.
- **Vault list**: The set of vaults available on the computer, shared by all app processes.
- **Shared model store**: Model files on disk that all app processes can see.
- **Presence marker**: The shared marker every running app process holds while it lives. A starting
  app process uses it to learn whether any other app process is running before it cleans up
  leftovers.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: After a close request, no result of that vault session reaches the interface, and the
  vault's content is no longer on screen within 1 second, in 100% of tested closes.
- **SC-002**: In 100% of tested closes, including with a streaming reply, a running download, a
  running tool and active voice capture, the old app process has ended within 4 seconds of the close
  request without any further action from the user.
- **SC-003**: In 100% of tests that unlock one vault after another, none of a defined set of marker
  values from the first vault (names, titles, drafts, selections, preferences, decisions, error
  texts) can be found in the second vault's interface or working state.
- **SC-004**: Across a full cycle of unlocking, using, failing to unlock with a wrong passphrase and
  closing, a known test passphrase appears in 0 log or diagnostic outputs.
- **SC-005**: In automated checks, every buffer that the app's own code uses for the passphrase is
  erased once it is released.
- **SC-006**: With two app processes each holding a different vault, closing or restarting one
  causes 0 interruptions in the other, including a chat reply that is streaming there.
- **SC-007**: Opening a vault that another app process holds is refused with a message the user can
  act on in 100% of attempts, and vault data is unchanged afterwards.
- **SC-008**: Returning to the chat view while a reply is streaming or a model is loading never
  shows an "operation in progress" error, in 100% of tested cases.
- **SC-009**: Two app processes installing the same model at the same time result in one valid
  installation in 100% of tested runs.
- **SC-010**: Starting a further app process while another creates a vault or downloads a model
  leaves that work intact and completing in 100% of tested runs.

## Assumptions

- Users accept that switching vaults takes a relaunch instead of happening inside the running app.
  This is the intended trade-off for complete isolation.
- The grace periods of about 1 second and about 3 seconds in total are the working values. They may
  be tuned during planning without changing the guarantees.
- Relaunching versus exiting after a close is a per-build choice. Development runs under developer
  tooling may need to exit instead of relaunching; planning decides how.
- Each app process shows one window. Planning verifies this.
- Work interrupted by a close is not resumed. Unsaved drafts are not preserved across a close. A
  reply cut by a close is kept up to what was received and marked as cancelled, exactly as when the
  user stops it; it is never shown as complete.
- The database engine keeps its own copy of the passphrase while a vault is open and erases it when
  the connection closes. This is accepted. Copies in the interface's runtime memory and in the
  messages passed between the interface and the backend cannot be erased by the app itself. They are
  bounded by the end of the process, which is why the process ends on close.
- Making the key wrapper erase itself when it is dropped requires a change in the storage library
  that holds it, which is developed separately.
- Separate read and write connections to the vault database are a possible later optimization and
  are out of scope. A second connection would need the passphrase at open time.
- Local models are unloaded when a vault closes, as they are today.
- Use without traces on a computer the user does not own is out of scope. A separate follow-up
  feature adds a portable mode in which all app data lives in a location the user chooses, for
  example a USB stick. Until then, model files, the vault file and other app data stay in the normal
  per-user data location of the computer.
- An end-to-end spike of the request gate and the bounded shutdown was built on the local branch
  `spike/vault-gateway` and informs planning. It is not a deliverable and is not merged.
- The related fix that keeps read-only lookups from taking the exclusive operation slot is already
  on this branch and is covered here by FR-025.
