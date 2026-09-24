import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import {
  PROCESS_END_LIMIT_MS,
  PROVIDER_CLOSE_LIMIT_MS,
  RELAUNCH_LIMIT_MS,
} from './close-promises.ts'

describe('close-promises', () => {
  it('holds the three fixed deadlines a scenario checks against', () => {
    assert.equal(PROCESS_END_LIMIT_MS, 4_000)
    assert.equal(PROVIDER_CLOSE_LIMIT_MS, 1_000)
    assert.equal(RELAUNCH_LIMIT_MS, 10_000)
  })
})
