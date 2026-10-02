import assert from "node:assert/strict";

process.env.SESSION_SECRET = "test-secret";

import { evaluateAccess } from "../src/lib/access";
import { issue, newToken, verify } from "../src/lib/session";

let passed = 0;
function check(name: string, fn: () => void): void {
  fn();
  passed++;
  console.log(`  ok  ${name}`);
}

check("newToken is random and long", () => {
  const a = newToken();
  const b = newToken();
  assert.notEqual(a, b);
  assert.ok(a.length >= 40, "token should be at least 40 chars");
});

check("session round-trips", () => {
  assert.equal(verify(issue(7)), 7);
  assert.equal(verify(issue(1234)), 1234);
});

check("tampered or malformed sessions are rejected", () => {
  const good = issue(7);
  const flipped = good.slice(0, -1) + (good.endsWith("A") ? "B" : "A");
  assert.equal(verify(flipped), null);
  assert.equal(verify("7.123.deadbeef"), null);
  assert.equal(verify("garbage"), null);
  assert.equal(verify(""), null);
  assert.equal(verify(undefined), null);
  assert.equal(verify(null), null);
});

check("access: active, future expiry -> ok", () => {
  const future = new Date(Date.now() + 60_000).toISOString();
  assert.equal(evaluateAccess({ active: 1, access_expires_at: future }), "ok");
});

check("access: no expiry -> never expires", () => {
  assert.equal(evaluateAccess({ active: 1, access_expires_at: null }), "ok");
});

check("access: past expiry -> expired", () => {
  const past = new Date(Date.now() - 60_000).toISOString();
  assert.equal(evaluateAccess({ active: 1, access_expires_at: past }), "expired");
});

check("access: inactive -> revoked (even if not expired)", () => {
  const future = new Date(Date.now() + 60_000).toISOString();
  assert.equal(evaluateAccess({ active: 0, access_expires_at: future }), "revoked");
});

console.log(`\n${passed} checks passed.`);
