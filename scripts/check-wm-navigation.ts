// Run with `pnpm check:wm-navigation` (spec 020-tab-navigation). Harness for the per-tab
// navigation modules under `src/lib/wm/` — like `check-wm-state.ts`, these modules avoid
// Nuxt auto-imports and use relative `.ts`-suffixed sibling imports (015 plan research R6) so they
// load standalone here.
//
// Sections: history reducers (navigation.ts) and the route matcher (routeMatch.ts). The action
// core, keybindings and the store-level navigation live in check-wm-actions.ts and
// check-wm-nav-store.ts, which `pnpm check:wm-navigation` runs together with this file (split
// to stay below the 500-line limit).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  backList,
  canGoBack,
  canGoForward,
  createHistory,
  currentLocation,
  forwardList,
  go,
  HISTORY_LIST_LIMIT,
  isLocationActive,
  locationsEqual,
  MAX_HISTORY_ENTRIES,
  parseLocation,
  push,
  removeEntry,
  replace,
  type TabHistory,
  withQuery,
} from '../src/lib/wm/navigation.ts'
import {
  locationTitle,
  matchRoute,
  type RoutePattern,
} from '../src/lib/wm/routeMatch.ts'

/** A history visited `/a` → `/b` → `/c`, positioned at `/c`. */
function abc(): TabHistory {
  return push(push(createHistory('/a'), '/b', 'A'), '/c', 'B')
}

function paths(history: TabHistory): string[] {
  return history.entries.map((entry) => entry.location.path)
}

// ---------------------------------------------------------------------------
// History reducers (navigation.ts)
// ---------------------------------------------------------------------------

test('createHistory starts with a single entry at the start location `/`', () => {
  const history = createHistory()
  assert.deepEqual(paths(history), ['/'])
  assert.equal(history.index, 0)
  assert.equal(canGoBack(history), false)
  assert.equal(canGoForward(history), false)
})

test('parseLocation normalizes the path and splits the query', () => {
  assert.deepEqual(parseLocation('models/hf/x/?sort=size&q=llama'), {
    path: '/models/hf/x',
    query: { sort: 'size', q: 'llama' },
  })
  assert.deepEqual(parseLocation('/'), { path: '/', query: {} })
  assert.deepEqual(parseLocation('//a//b/'), { path: '/a/b', query: {} })
})

test('locationsEqual ignores query key order but not values', () => {
  assert.ok(
    locationsEqual(parseLocation('/x?a=1&b=2'), parseLocation('/x?b=2&a=1')),
  )
  assert.ok(!locationsEqual(parseLocation('/x?a=1'), parseLocation('/x?a=2')))
  assert.ok(!locationsEqual(parseLocation('/x'), parseLocation('/y')))
})

test('push appends an entry, stores the leaving title and moves the position', () => {
  const history = abc()
  assert.deepEqual(paths(history), ['/a', '/b', '/c'])
  assert.equal(history.index, 2)
  assert.deepEqual(
    history.entries.map((entry) => entry.title),
    ['A', 'B', null],
  )
  assert.equal(currentLocation(history).path, '/c')
})

test('push drops all forward entries', () => {
  const back = go(abc(), -2)
  const pushed = push(back, '/d')
  assert.deepEqual(paths(pushed), ['/a', '/d'])
  assert.equal(pushed.index, 1)
  assert.equal(canGoForward(pushed), false)
})

test('push of a location equal to the current one is a no-op (FR-006)', () => {
  const history = push(createHistory('/x?a=1&b=2'), '/x?b=2&a=1')
  assert.equal(history.entries.length, 1)
  const same = createHistory('/y')
  assert.equal(push(same, '/y'), same)
})

test('replace changes only the current entry', () => {
  const history = replace(abc(), '/c?filter=on')
  assert.deepEqual(paths(history), ['/a', '/b', '/c'])
  assert.deepEqual(currentLocation(history).query, { filter: 'on' })
  assert.equal(history.index, 2)
})

test('go moves within bounds, clears the target title and stores the leaving one', () => {
  const back = go(abc(), -1, 'C')
  assert.equal(back.index, 1)
  assert.equal(back.entries[1]?.title, null)
  assert.equal(back.entries[2]?.title, 'C')
  const forward = go(back, 1)
  assert.equal(forward.index, 2)
})

test('go outside the bounds changes nothing (FR-007)', () => {
  const history = abc()
  assert.equal(go(history, 1), history)
  assert.equal(go(history, -3), history)
  assert.equal(go(history, 0), history)
})

test(`history keeps at most ${MAX_HISTORY_ENTRIES} entries, dropping the oldest`, () => {
  let history = createHistory('/0')
  for (let i = 1; i <= MAX_HISTORY_ENTRIES + 5; i++)
    history = push(history, `/${i}`)
  assert.equal(history.entries.length, MAX_HISTORY_ENTRIES)
  assert.equal(history.index, MAX_HISTORY_ENTRIES - 1)
  assert.equal(history.entries[0]?.location.path, '/6')
  assert.equal(currentLocation(history).path, `/${MAX_HISTORY_ENTRIES + 5}`)
})

test('removeEntry before the position shifts the position back', () => {
  const history = removeEntry(abc(), 0)
  assert.deepEqual(paths(history), ['/b', '/c'])
  assert.equal(currentLocation(history).path, '/c')
})

test('removeEntry at the position moves to the neighbor in travel direction', () => {
  const middle = go(abc(), -1)
  const goingBack = removeEntry(middle, 1, -1)
  assert.equal(currentLocation(goingBack).path, '/a')
  const goingForward = removeEntry(middle, 1, 1)
  assert.equal(currentLocation(goingForward).path, '/c')
})

test('removeEntry after the position keeps the current entry; the last entry is never removed', () => {
  const back = go(abc(), -2)
  const history = removeEntry(back, 2)
  assert.deepEqual(paths(history), ['/a', '/b'])
  assert.equal(currentLocation(history).path, '/a')
  const single = createHistory('/only')
  assert.equal(removeEntry(single, 0), single)
})

test(`backList and forwardList list nearest first, at most ${HISTORY_LIST_LIMIT} each`, () => {
  let history = createHistory('/0')
  for (let i = 1; i <= 20; i++) history = push(history, `/${i}`)
  const back = backList(history)
  assert.equal(back.length, HISTORY_LIST_LIMIT)
  assert.deepEqual(back[0], { steps: -1, entry: history.entries[19] })
  history = go(history, -3)
  assert.deepEqual(
    forwardList(history).map((item) => item.steps),
    [1, 2, 3],
  )
})

test('locations survive a JSON round trip (FR-001)', () => {
  const history = replace(abc(), '/c?x=1')
  assert.deepEqual(JSON.parse(JSON.stringify(history)), history)
})

// ---------------------------------------------------------------------------
// Route matcher (routeMatch.ts)
// ---------------------------------------------------------------------------

const SETTINGS_ROUTES: readonly RoutePattern[] = [
  {
    path: '/',
    children: [
      { path: '', titleKey: 'overview' },
      {
        path: 'models',
        titleKey: 'models',
        children: [
          { path: '', titleKey: 'models.list' },
          { path: 'hf/:repo', titleKey: 'models.detail' },
        ],
      },
      { path: 'thread/:id', titleKey: 'thread' },
    ],
  },
]

function chainKeys(path: string): (string | undefined)[] | null {
  return (
    matchRoute(SETTINGS_ROUTES, path)?.chain.map((route) => route.titleKey) ??
    null
  )
}

test('matchRoute resolves the index child of the root', () => {
  assert.deepEqual(chainKeys('/'), [undefined, 'overview'])
})

test('matchRoute resolves literal segments and nested index children', () => {
  assert.deepEqual(chainKeys('/models'), [undefined, 'models', 'models.list'])
})

test('matchRoute resolves nested params outer to inner and URL-decodes them', () => {
  const match = matchRoute(SETTINGS_ROUTES, '/models/hf/org%2Fname')
  assert.deepEqual(
    match?.chain.map((route) => route.titleKey),
    [undefined, 'models', 'models.detail'],
  )
  assert.deepEqual(match?.params, { repo: 'org/name' })
})

test('matchRoute normalizes a trailing slash', () => {
  assert.deepEqual(chainKeys('/models/'), [undefined, 'models', 'models.list'])
})

test('matchRoute returns null for an unknown path', () => {
  assert.equal(matchRoute(SETTINGS_ROUTES, '/nope'), null)
  assert.equal(matchRoute(SETTINGS_ROUTES, '/models/hf'), null)
})

test('an app without routes matches only `/`', () => {
  const implicit: readonly RoutePattern[] = [{ path: '/' }]
  assert.deepEqual(matchRoute(implicit, '/')?.chain.length, 1)
  assert.equal(matchRoute(implicit, '/x'), null)
})

// ---------------------------------------------------------------------------
// In-tab helpers used by useTabRouter/WmLink (US1 AS5, AS7)
// ---------------------------------------------------------------------------

test('setQuery-style replace keeps the history length and back returns to the previous view', () => {
  let history = push(createHistory('/'), '/models')
  const current = currentLocation(history)
  history = replace(history, withQuery(current, { sort: 'size' }))
  history = replace(
    history,
    withQuery(currentLocation(history), { q: 'llama' }),
  )
  assert.equal(history.entries.length, 2)
  assert.deepEqual(currentLocation(history).query, { sort: 'size', q: 'llama' })
  assert.equal(currentLocation(go(history, -1)).path, '/')
})

test('withQuery removes keys set to null', () => {
  const location = parseLocation('/x?a=1&b=2')
  assert.deepEqual(withQuery(location, { a: null, c: '3' }).query, {
    b: '2',
    c: '3',
  })
})

test('a prefix link marks nested locations active, a plain link only its own', () => {
  assert.ok(isLocationActive('/models/hf/x', '/models', true))
  assert.ok(!isLocationActive('/models/hf/x', '/models'))
  assert.ok(isLocationActive('/models/', '/models'))
  assert.ok(!isLocationActive('/modelsx', '/models', true))
  assert.ok(isLocationActive('/anything', '/', true))
})

// ---------------------------------------------------------------------------
// Location titles (US5, research R9)
// ---------------------------------------------------------------------------

test('locationTitle takes the deepest matched titleKey with its params', () => {
  assert.deepEqual(
    locationTitle(SETTINGS_ROUTES, '/models/hf/org%2Fx', 'app'),
    {
      key: 'models.detail',
      params: { repo: 'org/x' },
    },
  )
  assert.deepEqual(locationTitle(SETTINGS_ROUTES, '/models', 'app'), {
    key: 'models.list',
    params: {},
  })
})

test('locationTitle falls back to the app title without a routed title or match', () => {
  const routes: readonly RoutePattern[] = [
    {
      path: '/',
      children: [{ path: '' }, { path: 'thread/:id', titleKey: 'thread' }],
    },
  ]
  assert.deepEqual(locationTitle(routes, '/', 'app.chat'), {
    key: 'app.chat',
    params: {},
  })
  assert.deepEqual(locationTitle(routes, '/nope', 'app.chat'), {
    key: 'app.chat',
    params: {},
  })
  assert.equal(locationTitle(routes, '/thread/7', 'app.chat').key, 'thread')
})
