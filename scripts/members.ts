import { existsSync } from "node:fs";

if (existsSync(".env.local")) process.loadEnvFile(".env.local");

import { evaluateAccess } from "../src/lib/access";
import { targetLine } from "../src/lib/db";
import {
  createMember,
  deleteMember,
  extendMember,
  findMember,
  listMembers,
  resetInvite,
  setActive,
  setAdmin,
  type Member,
} from "../src/lib/members";

const APP_URL = process.env.APP_URL ?? "http://localhost:3000";
const invite = (token: string) => `${APP_URL}/i/${token}`;

function parseFlags(args: string[]): { flags: Record<string, string>; rest: string[] } {
  const flags: Record<string, string> = {};
  const rest: string[] = [];
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg.startsWith("--")) flags[arg.slice(2)] = args[++i] ?? "";
    else rest.push(arg);
  }
  return { flags, rest };
}

async function requireMember(idOrToken?: string): Promise<Member> {
  if (!idOrToken) fail("Which member? Pass an id or token (see `list`).");
  const member = await findMember(idOrToken!);
  if (!member) fail(`No member matching ${idOrToken}.`);
  return member!;
}

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

/**
 * Say which database every command is about to touch. There are two homes —
 * a local file and Turso Cloud — and it is easy to create a member in one while
 * the app reads the other. Printing this is cheap; discovering the split by
 * wondering why a fresh invite is "not valid" is not.
 */
function target(): string {
  return targetLine();
}

function line(member: Member): string {
  const status = evaluateAccess(member);
  const expires = member.access_expires_at ? member.access_expires_at.slice(0, 10) : "never";
  const username = member.username ?? "—";
  const role = member.is_admin ? "admin" : "user";
  return `${String(member.id).padStart(3)}  ${member.name.padEnd(16)}  ${username.padEnd(12)}  ${role.padEnd(6)}  ${status.padEnd(7)}  ${expires}`;
}

async function main(): Promise<void> {
  const [command, ...args] = process.argv.slice(2);
  const { flags, rest } = parseFlags(args);

  // Help is the one command that touches nothing, so it alone stays quiet.
  if (command && command !== "help" && !flags.help) console.log(target());

  switch (command) {
    case "add": {
      const name = rest[0];
      const username = flags.username;
      if (!name) fail("usage: npm run members -- add <name> --username <name> [--email x] [--days 30] [--note '...']");
      if (!username) fail("A --username is required — it's what the member signs in with.");
      const member = await createMember({
        name,
        username,
        email: flags.email,
        days: flags.days ? Number(flags.days) : undefined,
        note: flags.note,
        isAdmin: flags.admin !== undefined,
      });
      console.log(`Created member #${member.id} — ${member.name} (${member.username})${member.is_admin ? " [admin]" : ""}`);
      console.log(`Invite: ${invite(member.token)}`);
      if (member.access_expires_at) console.log(`Access until: ${member.access_expires_at.slice(0, 10)}`);
      console.log("Send them the invite link — it works once, and sets their password.");
      break;
    }
    case "list": {
      const all = await listMembers();
      if (all.length === 0) {
        console.log("No members yet.");
        break;
      }
      console.log("id   name              username      role    status   expires");
      for (const member of all) console.log(line(member));
      break;
    }
    case "invite":
      console.log(invite((await requireMember(rest[0])).token));
      break;
    case "reset": {
      const member = await requireMember(rest[0]);
      const updated = await resetInvite(member.id);
      console.log(`Fresh invite for ${updated!.name} (${updated!.username}) — their old password no longer works:`);
      console.log(`Invite: ${invite(updated!.token)}`);
      break;
    }
    case "revoke": {
      const member = await requireMember(rest[0]);
      await setActive(member.id, false);
      console.log(`Revoked ${member.name}. They are signed out everywhere.`);
      break;
    }
    case "promote": {
      const member = await requireMember(rest[0]);
      await setAdmin(member.id, true);
      console.log(`${member.name} is now an admin. There is nothing else to run here.`);
      break;
    }
    case "demote": {
      const member = await requireMember(rest[0]);
      await setAdmin(member.id, false);
      console.log(`${member.name} is no longer an admin.`);
      break;
    }
    case "enable": {
      const member = await requireMember(rest[0]);
      await setActive(member.id, true);
      console.log(`Enabled ${member.name}.`);
      break;
    }
    case "extend": {
      const member = await requireMember(rest[0]);
      const days = Number(flags.days ?? rest[1] ?? 30);
      const updated = await extendMember(member.id, days);
      console.log(`${updated!.name} now has access until ${updated!.access_expires_at!.slice(0, 10)}.`);
      break;
    }
    case "rm": {
      const member = await requireMember(rest[0]);
      await deleteMember(member.id);
      console.log(`Deleted ${member.name}.`);
      break;
    }
    default:
      console.log(
        [
          "Yunee — members admin",
          "",
          "  add <name> --username <name> [--email x] [--days N] [--note '...']   create a member + invite link",
          "  list                                                show members and status",
          "  invite <id|token>                                   print a member's invite link",
          "  reset <id|token>                                    new one-time link, clears the password",
          "  revoke <id|token>                                   turn access off (signs them out)",
          "  enable <id|token>                                   turn access back on",
          "  promote <id|token>                                  make them an admin",
          "  demote <id|token>                                   take admin away",
          "  extend <id|token> [--days N]                        add days (default 30)",
          "  rm <id|token>                                       delete a member",
          "",
          "Every command prints the database it is about to touch. Member management",
          "otherwise lives in the web app under /admin — the CLI is only for the very",
          "first setup, and for changing things the web UI cannot reach.",
          "",
          "Example: npm run members -- add Ada --username ada --email ada@example.com --days 120",
          "Bootstrap the first admin: npm run members -- promote 1",
        ].join("\n"),
      );
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
