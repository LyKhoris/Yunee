import assert from "node:assert/strict";

import { hashPassword, verifyPassword } from "../src/lib/password";

let passed = 0;
async function check(name: string, fn: () => Promise<void> | void): Promise<void> {
  await fn();
  passed++;
  console.log(`  ok  ${name}`);
}

async function main(): Promise<void> {
  await check("a hash verifies its own password", async () => {
    const stored = await hashPassword("correct horse battery staple");
    assert.equal(await verifyPassword("correct horse battery staple", stored), true);
  });

  await check("a wrong password does not verify", async () => {
    const stored = await hashPassword("correct horse battery staple");
    assert.equal(await verifyPassword("Correct horse battery staple", stored), false);
    assert.equal(await verifyPassword("", stored), false);
  });

  await check("the same password hashes differently each time (salted)", async () => {
    const a = await hashPassword("hunter2hunter2");
    const b = await hashPassword("hunter2hunter2");
    assert.notEqual(a, b);
    assert.equal(await verifyPassword("hunter2hunter2", a), true);
    assert.equal(await verifyPassword("hunter2hunter2", b), true);
  });

  await check("the stored form is self-describing scrypt", async () => {
    const stored = await hashPassword("whatever-goes-here");
    const parts = stored.split("$");
    assert.equal(parts[0], "scrypt");
    assert.equal(parts.length, 6);
  });

  await check("missing or malformed stored values are rejected", async () => {
    assert.equal(await verifyPassword("x", null), false);
    assert.equal(await verifyPassword("x", undefined), false);
    assert.equal(await verifyPassword("x", ""), false);
    assert.equal(await verifyPassword("x", "plaintext"), false);
    assert.equal(await verifyPassword("x", "scrypt$1$2$3$not-base64$also-not"), false);
    assert.equal(await verifyPassword("x", "scrypt$N$r$p$AAAA$AAAA"), false);
  });

  console.log(`\n${passed} checks passed.`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
