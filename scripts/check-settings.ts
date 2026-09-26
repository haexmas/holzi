// `pnpm check:settings` (spec 023-settings-app): the settings registry, header back, color
// scheme and the alias of the removed federation app (research R11). Pure modules, loaded with
// Node type-stripping like check-wm-state.ts.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import {
  categoryOf,
  headerBack,
  locationFor,
  overviewRows,
  SETTINGS_CATEGORIES,
  SETTINGS_LOCATIONS,
  settingsRoutePatterns,
} from '../src/lib/settings/registry.ts'
import {
  getAppDefinition,
  resolveAppAlias,
  tabTitleFor,
  WM_APPS,
} from '../src/lib/wm/apps.ts'
import { createHistory, push } from '../src/lib/wm/navigation.ts'
import { matchRoute } from '../src/lib/wm/routeMatch.ts'

const LOCATION_PATHS: Record<string, string> = {
  general: '/',
  appearance: '/appearance',
  models: '/models',
  'models.default': '/models/default',
  'models.installed': '/models/installed',
  'models.download': '/models/download',
  'models.download.search': '/models/download/search',
  'models.download.repo': '/models/download/repo/Qwen/Qwen2.5-GGUF',
  'models.speech': '/models/speech',
  agents: '/agents',
  'agents.providers': '/agents/providers',
  'agents.autonomy': '/agents/autonomy',
  'agents.denyRules': '/agents/deny-rules',
  federation: '/federation',
}

function flatKeys(tree: unknown, prefix = ''): Set<string> {
  const keys = new Set<string>()
  if (typeof tree !== 'object' || tree === null) return keys
  for (const [key, value] of Object.entries(tree)) {
    const path = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'string') keys.add(path)
    else for (const nested of flatKeys(value, path)) keys.add(nested)
  }
  return keys
}

function localeKeys(locale: 'de' | 'en'): Set<string> {
  const url = new URL(`../src/i18n/locales/${locale}.json`, import.meta.url)
  return flatKeys(JSON.parse(readFileSync(url, 'utf8')))
}

test('five categories in the order of FR-005, each at its own location', () => {
  assert.deepEqual(
    SETTINGS_CATEGORIES.map((category) => [category.id, category.path]),
    [
      ['general', '/'],
      ['appearance', '/appearance'],
      ['models', '/models'],
      ['agents', '/agents'],
      ['federation', '/federation'],
    ],
  )
  for (const category of SETTINGS_CATEGORIES) {
    assert.ok(category.icon.startsWith('lucide:'), category.id)
    assert.equal(locationFor(category.path)?.location.id, category.id)
  }
})

test('locations have unique ids and patterns, a category, and titles', () => {
  const ids = SETTINGS_LOCATIONS.map((location) => location.id)
  const patterns = SETTINGS_LOCATIONS.map((location) => location.pattern)
  assert.equal(new Set(ids).size, ids.length)
  assert.equal(new Set(patterns).size, patterns.length)
  assert.deepEqual([...ids].sort(), Object.keys(LOCATION_PATHS).sort())
  const categoryIds = new Set(
    SETTINGS_CATEGORIES.map((category) => category.id),
  )
  for (const location of SETTINGS_LOCATIONS) {
    assert.ok(categoryIds.has(location.category), location.id)
    assert.ok(location.titleKey.length > 0, location.id)
    if (location.parent) {
      const parent = SETTINGS_LOCATIONS.find(
        (candidate) => candidate.id === location.parent,
      )
      assert.ok(parent, `${location.id} → ${location.parent}`)
      assert.equal(parent.category, location.category, location.id)
    }
    if (location.overviewRow) {
      assert.ok(location.icon?.startsWith('lucide:'), location.id)
      assert.ok(location.descriptionKey, location.id)
    }
  }
})

test('overview rows of models and agents; general, appearance and federation have none', () => {
  const rows = (id: Parameters<typeof overviewRows>[0]) =>
    overviewRows(id).map((location) => location.id)
  assert.deepEqual(rows('models'), [
    'models.default',
    'models.installed',
    'models.download',
    'models.speech',
  ])
  assert.deepEqual(rows('agents'), [
    'agents.providers',
    'agents.autonomy',
    'agents.denyRules',
  ])
  assert.deepEqual(rows('general'), [])
  assert.deepEqual(rows('appearance'), [])
  assert.deepEqual(rows('federation'), [])
})

test('the route patterns resolve every location, with the repo params', () => {
  const routes = settingsRoutePatterns()
  for (const [id, path] of Object.entries(LOCATION_PATHS)) {
    const match = matchRoute(routes, path)
    assert.ok(match, path)
    assert.equal(match.chain.length, 2, path)
    assert.equal(locationFor(path)?.location.id, id, path)
  }
  assert.deepEqual(
    locationFor('/models/download/repo/Qwen/Qwen2.5-GGUF')?.params,
    {
      owner: 'Qwen',
      name: 'Qwen2.5-GGUF',
    },
  )
  assert.equal(locationFor('/nope'), undefined)
  assert.equal(locationFor('/models/nope'), undefined)
})

test('categoryOf maps every location to its category', () => {
  assert.equal(categoryOf('/'), 'general')
  assert.equal(categoryOf('/models/download/repo/Qwen/Qwen2.5-GGUF'), 'models')
  assert.equal(categoryOf('/agents/deny-rules'), 'agents')
  assert.equal(categoryOf('/federation'), 'federation')
  assert.equal(categoryOf('/nope'), undefined)
})

test('every registry text exists in German and English (FR-020)', () => {
  const keys = [
    ...SETTINGS_CATEGORIES.map((category) => category.titleKey),
    ...SETTINGS_LOCATIONS.flatMap((location) =>
      location.descriptionKey
        ? [location.titleKey, location.descriptionKey]
        : [location.titleKey],
    ),
    'settings.back',
  ]
  for (const locale of ['de', 'en'] as const) {
    const available = localeKeys(locale)
    const missing = keys.filter((key) => !available.has(key))
    assert.deepEqual(missing, [], locale)
  }
})

test('header back acts as tab back when the parent is the previous entry', () => {
  const fromSearch = push(
    push(createHistory('/models/download'), '/models/download/search?q=qwen'),
    '/models/download/repo/Qwen/Qwen2.5-GGUF',
  )
  assert.deepEqual(headerBack(fromSearch, '/models/download/search'), {
    kind: 'back',
  })
  const fromOverview = push(createHistory('/models'), '/models/installed')
  assert.deepEqual(headerBack(fromOverview, '/models'), { kind: 'back' })
})

test('header back navigates to the parent otherwise, also after a deep link', () => {
  const fromElsewhere = push(createHistory('/agents'), '/models/installed')
  assert.deepEqual(headerBack(fromElsewhere, '/models'), {
    kind: 'push',
    path: '/models',
  })
  const deepLink = createHistory('/models/download/repo/Qwen/Qwen2.5-GGUF')
  assert.deepEqual(headerBack(deepLink, '/models/download/search'), {
    kind: 'push',
    path: '/models/download/search',
  })
})

test('the settings tab keeps the app title; other apps show the location title', () => {
  const routed = {
    key: 'settings.locations.models.installed.title',
    params: {},
  }
  const settings = getAppDefinition('system.settings')
  assert.equal(settings?.tabTitle, 'app')
  assert.deepEqual(tabTitleFor(settings, routed), {
    key: 'wm.apps.settings',
    params: {},
  })
  const thread = { key: 'wm.chat.thread', params: { id: 't1' } }
  assert.deepEqual(tabTitleFor(getAppDefinition('system.chat'), thread), thread)
  assert.deepEqual(tabTitleFor(undefined, thread), thread)
})

test('the removed federation app opens the settings at the federation category (R11)', () => {
  assert.deepEqual(resolveAppAlias('system.federation'), {
    appId: 'system.settings',
    at: '/federation',
  })
  assert.deepEqual(resolveAppAlias('system.chat'), {
    appId: 'system.chat',
    at: null,
  })
  assert.ok(!WM_APPS.some((app) => app.id === 'system.federation'))
})
