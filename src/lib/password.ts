import { randomBytes, scrypt as scryptCb, timingSafeEqual } from "node:crypto";

/**
 * Password hashing with scrypt — built into Node, so no dependency and no native
 * build.
 *
 * The stored form is self-describing so the cost parameters can be raised later
 * without breaking existing hashes:
 *
 *   scrypt$N$r$p$<salt base64>$<hash base64>
 *
 * Verify reads the parameters back out of the stored value, so an old hash keeps
 * working after the defaults below change.
 */
const N = 16_384;
const R = 8;
const P = 1;
const KEYLEN = 32;
const MAXMEM = 64 * 1024 * 1024;

function scrypt(
  password: string,
  salt: Buffer,
  keylen: number,
  opts: { N: number; r: number; p: number },
): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    scryptCb(password, salt, keylen, { ...opts, maxmem: MAXMEM }, (error, key) =>
      error ? reject(error) : resolve(key),
    );
  });
}

/** Hash a password with a fresh random salt, returning its portable encoding. */
export async function hashPassword(password: string): Promise<string> {
  const salt = randomBytes(16);
  const key = await scrypt(password.normalize("NFKC"), salt, KEYLEN, { N, r: R, p: P });
  return ["scrypt", N, R, P, salt.toString("base64"), key.toString("base64")].join("$");
}

/** Constant-time check. A missing or malformed stored value is simply no match. */
export async function verifyPassword(
  password: string,
  stored: string | null | undefined,
): Promise<boolean> {
  if (!stored) return false;
  const parts = stored.split("$");
  if (parts.length !== 6 || parts[0] !== "scrypt") return false;

  const params = { N: Number(parts[1]), r: Number(parts[2]), p: Number(parts[3]) };
  if (
    !Number.isInteger(params.N) ||
    !Number.isInteger(params.r) ||
    !Number.isInteger(params.p)
  ) {
    return false;
  }

  const salt = Buffer.from(parts[4], "base64");
  const expected = Buffer.from(parts[5], "base64");
  if (salt.length === 0 || expected.length === 0) return false;

  // scrypt rejects parameters it considers unsafe; a stored value that provokes
  // that is malformed, not a match.
  try {
    const key = await scrypt(password.normalize("NFKC"), salt, expected.length, params);
    return key.length === expected.length && timingSafeEqual(key, expected);
  } catch {
    return false;
  }
}
