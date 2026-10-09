// Which app a name opens (spec 046-agent-choice-prompt, FR-001–FR-004, research R5): the agent passes
// what the user said ("haex-mial", "Einstellungen") or an id it guessed ("system.notes"). Pure, so
// `scripts/check-wm-app-match.ts` runs it without vue; the caller passes the translation. App names
// are short, so a plain edit distance ranks them better than Fuse, which scores the best substring.
import { fold } from '../search/fold.ts'
import {
  getAppDefinition,
  resolveAppAlias,
  type AppDefinition,
} from './apps.ts'

/** An app offered when the input fits no app clearly. */
export type AppCandidate = {
  appId: string
  title: string
  /** Why it cannot open on this device; the choice shows it disabled. */
  unavailableKey?: string
}

export type AppMatch =
  | { kind: 'exact'; appId: string; at: string | null }
  | { kind: 'choice'; candidates: AppCandidate[] }

const ID_PREFIX = /^(system|extension)\./
const MAX_CANDIDATES = 5
/** Scores run from 0 (identical) to 1. A name scoring above this is no candidate at all. */
const CANDIDATE_SCORE = 0.5
/** A best hit counts as clear when it scores at most this… */
const CLEAR_SCORE = 0.2
/** …and the next hit is this much further away. Calibrated in `check-wm-app-match.ts`: a swapped,
 * missing or extra letter in a name stays clear; a prefix several apps share ("haex") does not. */
const CLEAR_GAP = 0.1

type Entry = { app: AppDefinition; title: string; idRest: string }

/** The app `input` names. An app id or a replaced one opens directly; otherwise the input is read as
 * a name, ignoring case, accents and an id prefix a model may have guessed. */
export function matchApp(
  input: string,
  apps: readonly AppDefinition[],
  titleOf: (app: AppDefinition) => string,
): AppMatch {
  const alias = resolveAppAlias(input)
  if (getAppDefinition(alias.appId, apps)) {
    return { kind: 'exact', appId: alias.appId, at: alias.at }
  }

  const query = fold(input.trim()).replace(ID_PREFIX, '')
  const entries: Entry[] = apps.map((app) => ({
    app,
    title: fold(titleOf(app)),
    // An extension's id is a UUID; only holzi's own ids carry a word ("settings").
    idRest: app.id.startsWith('system.') ? app.id.replace(ID_PREFIX, '') : '',
  }))

  const named = entries.filter(
    (entry) => entry.title === query || entry.idRest === query,
  )
  if (named.length === 1) return exactMatch(named[0]!.app)

  const hits = entries
    .map((entry) => ({
      app: entry.app,
      score: Math.min(
        nameScore(query, entry.title),
        entry.idRest ? nameScore(query, entry.idRest) : 1,
      ),
    }))
    .filter((hit) => query !== '' && hit.score <= CANDIDATE_SCORE)
    .sort((a, b) => a.score - b.score)
  const [best, second] = hits
  if (
    best &&
    best.score <= CLEAR_SCORE &&
    (!second || second.score - best.score >= CLEAR_GAP)
  ) {
    return exactMatch(best.app)
  }
  return {
    kind: 'choice',
    candidates: hits
      .slice(0, MAX_CANDIDATES)
      .map((hit) => candidate(hit.app, titleOf)),
  }
}

/** How far `query` is from `name`, both folded. One contained in the other scores low, shorter
 * names first ("note" fits "Notes" better than "Notes Pro"); otherwise the edit distance relative to
 * the longer of the two. */
function nameScore(query: string, name: string): number {
  if (name === '') return 1
  if (name.includes(query)) return 0.05 + 0.1 * (1 - query.length / name.length)
  if (query.includes(name)) return 0.05 + 0.1 * (1 - name.length / query.length)
  return editDistance(query, name) / Math.max(query.length, name.length)
}

/** Levenshtein distance where swapping two neighbouring letters counts as one edit ("mial" →
 * "mail"), the most common typo. */
function editDistance(a: string, b: string): number {
  const rows = Array.from({ length: a.length + 1 }, (_, i) =>
    Array.from({ length: b.length + 1 }, (_, j) =>
      i === 0 ? j : j === 0 ? i : 0,
    ),
  )
  for (let i = 1; i <= a.length; i++) {
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1
      rows[i]![j] = Math.min(
        rows[i - 1]![j]! + 1,
        rows[i]![j - 1]! + 1,
        rows[i - 1]![j - 1]! + cost,
      )
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) {
        rows[i]![j] = Math.min(rows[i]![j]!, rows[i - 2]![j - 2]! + 1)
      }
    }
  }
  return rows[a.length]![b.length]!
}

function exactMatch(app: AppDefinition): AppMatch {
  return { kind: 'exact', appId: app.id, at: null }
}

function candidate(
  app: AppDefinition,
  titleOf: (app: AppDefinition) => string,
): AppCandidate {
  return {
    appId: app.id,
    title: titleOf(app),
    ...(app.unavailableKey ? { unavailableKey: app.unavailableKey } : {}),
  }
}
