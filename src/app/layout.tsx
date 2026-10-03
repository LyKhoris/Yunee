import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { Fredoka, Nunito, Geist_Mono } from "next/font/google";
import "./globals.css";

// Fredoka carries the friendly display voice; Nunito does the body work at small
// sizes; Geist Mono stays for timestamps and code. Ported from Foxi (AGENTS.md).
const fredoka = Fredoka({
  variable: "--font-fredoka",
  subsets: ["latin"],
  weight: ["400", "500", "600", "700"],
});

const nunito = Nunito({
  variable: "--font-nunito",
  subsets: ["latin"],
  weight: ["400", "600", "700", "800"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  applicationName: "Yunee",
  title: "Yunee",
  description: "A private lecture companion. Your lectures, remembered.",
};

export const viewport: Viewport = {
  themeColor: "#14120f",
  colorScheme: "dark",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html
      lang="en"
      className={`dark ${fredoka.variable} ${nunito.variable} ${geistMono.variable} h-full antialiased`}
    >
      <body className="min-h-full flex flex-col">{children}</body>
    </html>
  );
}
