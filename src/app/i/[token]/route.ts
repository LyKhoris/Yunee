import { NextResponse } from "next/server";
import { getMemberByToken } from "@/lib/members";
import { setSession } from "@/server/session";

export async function GET(
  request: Request,
  { params }: { params: Promise<{ token: string }> },
): Promise<NextResponse> {
  const { token } = await params;
  const member = await getMemberByToken(token);
  if (!member) {
    return new NextResponse("This invitation link is not valid.", { status: 404 });
  }
  await setSession(member.id);
  return NextResponse.redirect(new URL("/", request.url));
}
