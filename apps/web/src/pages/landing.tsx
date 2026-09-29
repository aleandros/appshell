import { ButtonAnchor, Card } from '@appshell/ui';
import { ButtonLink } from '../components/links';
import { Link } from '@tanstack/react-router';
import {
  ArrowRight,
  Check,
  CheckCircle2,
  CreditCard,
  LayoutDashboard,
  LockKeyhole,
  Settings2,
  ShieldCheck,
  Sparkles,
  Users,
} from 'lucide-react';
import { Logo, Plant, Badge } from '../components/ui';
import { ThemeSwitcher } from '../components/theme';
import { brand } from '../config/brand';
export function Landing() {
  return (
    <div className="mx-auto max-w-7xl px-5 sm:px-10">
      <header className="flex min-h-24 flex-wrap items-center justify-between gap-4 border-b py-4">
        <Link to="/" aria-label="AppShell home">
          <Logo />
        </Link>
        <nav
          className="hidden items-center gap-8 text-sm text-muted md:flex"
          aria-label="Main navigation"
        >
          <a href="#possibilities" className="hover:text-foreground">
            Why AppShell
          </a>
          <a href="#pricing" className="hover:text-foreground">
            Plans
          </a>
        </nav>
        <div className="flex items-center gap-3">
          <ButtonLink variant="quiet" to="/login">
            Sign in
          </ButtonLink>
          <ButtonLink variant="primary" to="/signup">
            Get started <ArrowRight size={16} />
          </ButtonLink>
        </div>
      </header>
      <main>
        <section className="grid items-center gap-12 py-16 lg:grid-cols-[1fr_1.04fr] lg:gap-16 lg:py-24">
          <div>
            <Badge>
              <span className="h-1.5 w-1.5 rounded-full bg-primary" /> ROOM FOR YOUR NEXT CHAPTER
            </Badge>
            <h1 className="mt-7 text-5xl leading-[1.1] font-semibold tracking-[-.045em] sm:text-6xl lg:text-[72px]">
              Good things
              <br />
              start with
              <br />
              <span className="text-primary">a little space.</span>
            </h1>
            <p className="mt-7 max-w-md text-lg leading-relaxed text-muted">
              Bring your people, your ideas, and your next big thing together. A thoughtful
              workspace that grows with you.
            </p>
            <div className="mt-9 flex flex-wrap gap-3">
              <ButtonLink variant="primary" to="/signup" className="px-6">
                Create your workspace <ArrowRight size={17} />
              </ButtonLink>
              <ButtonAnchor variant="secondary" href="#possibilities">
                Take a look
              </ButtonAnchor>
            </div>
            <p className="mt-5 flex items-center gap-2 text-xs text-muted">
              <Check size={14} /> Free to start <span className="mx-1">·</span> No credit card
              needed
            </p>
          </div>
          <div className="relative rounded-[28px] border bg-accent/35 p-5 sm:p-8">
            <span className="eyebrow mb-5 block">A home for what comes next</span>
            <Card as="div" className="relative overflow-hidden">
              <div className="flex items-center justify-between border-b p-5">
                <div className="flex items-center gap-2">
                  <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent text-accent-foreground">
                    <Sparkles size={16} />
                  </span>
                  <span className="text-xs font-semibold">Your workspace</span>
                </div>
                <div className="flex gap-1.5">
                  {[1, 2, 3].map((v) => (
                    <span key={v} className="h-2 w-2 rounded-full bg-border" />
                  ))}
                </div>
              </div>
              <div className="p-6">
                <div className="flex justify-between">
                  <div>
                    <p className="text-xs text-muted">MAKE YOURSELF AT HOME</p>
                    <h2 className="mt-2 text-2xl font-semibold tracking-tight">
                      A fresh beginning.
                    </h2>
                    <p className="mt-2 text-xs text-muted">
                      A little structure. A lot of possibility.
                    </p>
                  </div>
                </div>
                <div className="relative mt-5 flex h-52 items-center justify-center overflow-hidden rounded-xl bg-surface-muted">
                  <div className="absolute inset-0 grid-paper opacity-35" />
                  <div className="relative -translate-y-1 scale-75">
                    <Plant />
                  </div>
                  <span className="absolute right-3 bottom-3 rounded-lg border bg-surface/90 px-3 py-2 text-[10px] text-muted">
                    Made for growing ideas ↗
                  </span>
                </div>
                <div className="mt-5 space-y-3">
                  {['Make it yours', 'Bring your people', 'Build something great'].map((s, i) => (
                    <div key={s} className="flex items-center gap-3 rounded-lg border p-3 text-xs">
                      <CheckCircle2
                        size={16}
                        className={i === 0 ? 'text-primary' : 'text-border'}
                      />
                      <span>{s}</span>
                      {i === 0 && <span className="ml-auto text-[10px] text-primary">All set</span>}
                    </div>
                  ))}
                </div>
              </div>
            </Card>
            <div className="absolute -right-2 -bottom-4 flex items-center gap-3 rounded-xl border bg-surface px-5 py-4 shadow-lg sm:-right-4">
              <span className="flex h-9 w-9 items-center justify-center rounded-full bg-accent text-primary">
                <Users size={18} />
              </span>
              <div>
                <p className="text-xs font-semibold">Better, together.</p>
                <p className="mt-1 text-[10px] text-muted">Your team belongs here.</p>
              </div>
            </div>
          </div>
        </section>
        <section id="possibilities" className="border-t py-16">
          <div className="mb-9 flex flex-wrap items-end justify-between gap-4">
            <div>
              <p className="eyebrow">LESS FRICTION, MORE POSSIBILITY</p>
              <h2 className="mt-3 text-3xl font-semibold tracking-tight">
                The essentials, thoughtfully done.
              </h2>
            </div>
            <p className="max-w-sm leading-relaxed text-muted">
              A comfortable starting point for a team of one, or a whole room of possibility.
            </p>
          </div>
          <div className="grid gap-5 md:grid-cols-3">
            {[
              {
                icon: Users,
                title: 'A place for your people',
                description:
                  'Invite your collaborators. Keep work organized with separate workspaces and clear roles.',
              },
              {
                icon: ShieldCheck,
                title: 'Peace of mind, built in',
                description:
                  'Verified accounts, private workspaces, and account recovery when you need it.',
              },
              {
                icon: Settings2,
                title: 'Make yourself at home',
                description:
                  'Light, dark, or whatever your system prefers. A familiar experience on every screen.',
              },
            ].map(({ icon: Icon, title, description }) => (
              <Card as="article" key={title} className="p-7">
                <span className="mb-6 flex h-11 w-11 items-center justify-center rounded-xl bg-surface-muted text-primary">
                  <Icon size={21} />
                </span>
                <h3 className="text-lg font-semibold">{title}</h3>
                <p className="mt-3 text-sm leading-7 text-muted">{description}</p>
              </Card>
            ))}
          </div>
        </section>
        <section
          id="pricing"
          className="mb-16 grid gap-8 rounded-3xl border bg-surface p-7 sm:p-10 lg:grid-cols-2"
        >
          <div>
            <p className="eyebrow">SMALL STARTS WELCOME</p>
            <h2 className="mt-4 text-3xl font-semibold tracking-tight">
              Start free. Grow at your pace.
            </h2>
            <p className="mt-4 max-w-md leading-7 text-muted">
              Your first workspace includes up to {brand.freeSeats} people and all the essentials.
              Larger teams can move to Pro when billing is enabled for their installation.
            </p>
            <ButtonLink variant="primary" to="/signup" className="mt-6">
              Find your space <ArrowRight size={16} />
            </ButtonLink>
          </div>
          <div className="grid grid-cols-2 gap-3">
            {[
              { icon: Users, label: '3 team members' },
              { icon: LockKeyhole, label: 'Private by default' },
              { icon: LayoutDashboard, label: 'Unlimited possibility' },
              { icon: CreditCard, label: 'No card to start' },
            ].map(({ icon: Icon, label }) => (
              <div
                key={label}
                className="flex flex-col justify-center gap-3 rounded-2xl bg-background p-5"
              >
                <Icon size={21} className="text-primary" />
                <span className="text-sm font-medium">{label}</span>
              </div>
            ))}
          </div>
        </section>
      </main>
      <footer className="flex flex-wrap items-center justify-between gap-5 border-t py-7">
        <div className="flex items-center gap-4">
          <Logo compact />
          <span className="text-xs text-muted">
            {brand.name} · {brand.tagline}
          </span>
        </div>
        <ThemeSwitcher />
      </footer>
    </div>
  );
}
