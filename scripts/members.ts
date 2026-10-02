import { existsSync } from "node:fs";

if (existsSync(".env.local")) process.loadEnvFile(".env.local");

import { evaluateAccess } from "../src/lib/access";
import {
  createMember,
  deleteMember,
  extendMember,
  findMember,
  listMembers,
  setActive,
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

function line(member: Member): string {
  const status = evaluateAccess(member);
  const expires = member.access_expires_at ? member.access_expires_at.slice(0, 10) : "never";
  return `${String(member.id).padStart(3)}  ${member.name.padEnd(18)}  ${status.padEnd(7)}  ${expires}`;
}

async function main(): Promise<void> {
  const [command, ...args] = process.argv.slice(2);
  const { flags, rest } = parseFlags(args);

  switch (command) {
    case "add": {
      const name = rest[0];
      if (!name) fail("usage: npm run members -- add <name> [--email x] [--days 30] [--note '...']");
      const member = await createMember({
        name,
        email: flags.email,
        days: flags.days ? Number(flags.days) : undefined,
        note: flags.note,
      });
      console.log(`Created member #${member.id} — ${member.name}`);
      console.log(`Invite: ${invite(member.token)}`);
      if (member.access_expires_at) console.log(`Access until: ${member.access_expires_at.slice(0, 10)}`);
      break;
    }
    case "list": {
      const all = await listMembers();
      if (all.length === 0) {
        console.log("No members yet.");
        break;
      }
      console.log("id   name                status   expires");
      for (const member of all) console.log(line(member));
      break;
    }
    case "invite":
      console.log(invite((await requireMember(rest[0])).token));
      break;
    case "revoke": {
      const member = await requireMember(rest[0]);
      await setActive(member.id, false);
      console.log(`Revoked ${member.name}.`);
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
          "  add <name> [--email x] [--days N] [--note '...']   create a member + invite link",
          "  list                                                show members and status",
          "  invite <id|token>                                   print a member's invite link",
          "  revoke <id|token>                                   turn access off",
          "  enable <id|token>                                   turn access back on",
          "  extend <id|token> [--days N]                        add days (default 30)",
          "  rm <id|token>                                       delete a member",
          "",
          "Example: npm run members -- add Ada --email ada@example.com --days 120",
        ].join("\n"),
      );
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
