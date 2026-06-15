import type { Metadata } from "next";
import Link from "next/link";
import {
  LayoutDashboard,
  Bot,
  Activity,
  Menu,
  X,
} from "lucide-react";
import "./globals.css";

export const metadata: Metadata = {
  title: "AgenticOS — Agent Platform",
  description: "Build, deploy, and monitor AI agents",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <body className="min-h-screen bg-surface-950">
        <Navbar />
        <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
          {children}
        </main>
      </body>
    </html>
  );
}

function Navbar() {
  return (
    <nav className="sticky top-0 z-50 bg-surface-950/80 backdrop-blur-xl border-b border-surface-800">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          <div className="flex items-center gap-8">
            <Link href="/" className="flex items-center gap-2.5 group">
              <div className="w-8 h-8 rounded-lg bg-brand-600 flex items-center justify-center group-hover:bg-brand-500 transition-colors">
                <span className="text-white font-bold text-sm">A</span>
              </div>
              <span className="text-lg font-semibold text-white">
                AgenticOS
              </span>
            </Link>
            <div className="hidden md:flex items-center gap-1">
              <NavLink href="/" icon={<LayoutDashboard size={16} />}>
                Dashboard
              </NavLink>
              <NavLink href="/agents" icon={<Bot size={16} />}>
                Agents
              </NavLink>
              <NavLink href="/runs" icon={<Activity size={16} />}>
                Runs
              </NavLink>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <span className="hidden sm:inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-green-500/10 text-green-400 text-xs font-medium border border-green-500/20">
              <span className="w-1.5 h-1.5 rounded-full bg-green-400 animate-pulse-slow" />
              All Systems Nominal
            </span>
            <div className="w-8 h-8 rounded-full bg-brand-600 flex items-center justify-center text-white text-sm font-medium">
              U
            </div>
          </div>
        </div>
      </div>
    </nav>
  );
}

function NavLink({
  href,
  icon,
  children,
}: {
  href: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Link
      href={href}
      className="flex items-center gap-2 px-3 py-2 rounded-lg text-surface-400 hover:text-surface-200 hover:bg-surface-800 transition-colors text-sm font-medium"
    >
      {icon}
      {children}
    </Link>
  );
}
