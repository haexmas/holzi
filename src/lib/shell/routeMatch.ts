// Route matching for per-tab navigation (spec 020-tab-navigation, T010, research R4). An app
// declares nested route patterns; a path resolves to the chain of matching patterns from the
// outermost to the innermost plus the collected `:param` values. Pure — the Vue side attaches
// components to these patterns in `components/shell/appRoutes.ts`.
import { normalizePath } from './navigation.ts'

/** `path` is relative to the parent pattern (roots start with `/`); segments are literals or
 * `:name`; `''` is the parent's index child. */
export type RoutePattern = {
  path: string
  titleKey?: string
  children?: readonly RoutePattern[]
}

export type RouteMatch<R extends RoutePattern = RoutePattern> = {
  chain: R[]
  params: Record<string, string>
}

function patternSegments(path: string): string[] {
  return path.split('/').filter((segment) => segment.length > 0)
}

function decode(segment: string): string {
  try {
    return decodeURIComponent(segment)
  } catch {
    return segment
  }
}

function matchLevel<R extends RoutePattern>(
  routes: readonly R[],
  segments: readonly string[],
): RouteMatch<R> | null {
  for (const route of routes) {
    const pattern = patternSegments(route.path)
    if (pattern.length > segments.length) continue
    const params: Record<string, string> = {}
    let ok = true
    pattern.forEach((part, i) => {
      const actual = segments[i] ?? ''
      if (part.startsWith(':')) params[part.slice(1)] = decode(actual)
      else if (part !== actual) ok = false
    })
    if (!ok) continue
    const rest = segments.slice(pattern.length)
    const children = (route.children ?? []) as readonly R[]
    if (children.length > 0) {
      const inner = matchLevel(children, rest)
      if (inner) {
        return {
          chain: [route, ...inner.chain],
          params: { ...params, ...inner.params },
        }
      }
    }
    if (rest.length === 0) return { chain: [route], params }
  }
  return null
}

/** `null` when no pattern chain consumes the whole path (FR-014 then falls back to `/`). */
export function matchRoute<R extends RoutePattern>(
  routes: readonly R[],
  path: string,
): RouteMatch<R> | null {
  return matchLevel(routes, patternSegments(normalizePath(path)))
}
