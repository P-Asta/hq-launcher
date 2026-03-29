import { useEffect, useMemo, useState } from 'react'
import {
  ArrowUpRight,
  BadgeCheck,
  Download,
  FolderArchive,
  Gamepad2,
  Github,
  LoaderCircle,
  Monitor,
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  TerminalSquare,
  Wrench,
} from 'lucide-react'

const RELEASES_PAGE_URL = 'https://github.com/p-asta/hq-launcher/releases/latest'
const REPOSITORY_URL = 'https://github.com/p-asta/hq-launcher'
const RELEASES_API_URL =
  'https://api.github.com/repos/p-asta/hq-launcher/releases/latest'

const PLACEHOLDER_IMAGES = {
  main: '/placeholders/launcher-main-shot.svg',
  practice: '/placeholders/launcher-practice-shot.svg',
  config: '/placeholders/launcher-config-shot.svg',
}

const highlightItems = [
  {
    icon: ShieldCheck,
    label: 'Steam authentication',
  },
  {
    icon: Gamepad2,
    label: 'HQ / Brutal / Wesley presets',
  },
  {
    icon: SlidersHorizontal,
    label: 'Mods + config editor',
  },
  {
    icon: RefreshCw,
    label: 'Auto updates',
  },
]

const featureSections = [
  {
    eyebrow: 'MAIN INTERFACE',
    title: 'Keep versions, launch status, and actions in one screen',
    description:
      'HQ Launcher is built so the setup loop stays short. Installed versions, download flow, and launch controls all live in the same primary surface.',
    bullets: [
      'Steam login is part of the launcher flow instead of a separate step.',
      'Install, switch, or remove supported versions from the same place.',
      'See the current state before launching without bouncing between tools.',
    ],
    image: PLACEHOLDER_IMAGES.main,
    alt: 'Placeholder main interface screenshot',
  },
  {
    eyebrow: 'PRACTICE PRESETS',
    title: 'Jump into HQ, Brutal, or Wesley practice without manual prep',
    description:
      'Practice runs are treated like real presets. The launcher prepares compatible practice mods for the selected game version so repeat attempts are faster to start.',
    bullets: [
      'Normal Practice, Brutal Practice, and Wesley presets are clearly separated.',
      'Missing compatible practice mods can be installed for that run automatically.',
      'Version-aware setup reduces the usual compatibility guesswork.',
    ],
    image: PLACEHOLDER_IMAGES.practice,
    alt: 'Placeholder practice preset screenshot',
  },
  {
    eyebrow: 'MODS & CONFIG',
    title: 'Search mods and edit configs without leaving the launcher',
    description:
      'The main branch already focuses on combining mod management and config editing, so this landing page now leaves obvious placeholder slots for the real UI shots you want to capture.',
    bullets: [
      'Enable or disable mods from one searchable list.',
      'Adjust BepInEx configuration values in a side-by-side editing flow.',
      'Keep progress and update work visible while the launcher handles background tasks.',
    ],
    image: PLACEHOLDER_IMAGES.config,
    alt: 'Placeholder config editor screenshot',
  },
]

function formatDate(value) {
  if (!value) {
    return ''
  }

  return new Intl.DateTimeFormat('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  }).format(new Date(value))
}

function findAsset(assets, extension) {
  const normalizedExtension = extension.toLowerCase()

  return (
    assets.find((asset) =>
      (asset?.name ?? '').toLowerCase().endsWith(normalizedExtension),
    ) ?? null
  )
}

function SectionLabel({ children, light = false }) {
  return (
    <div
      className={light
        ? 'text-[11px] uppercase tracking-[0.22em] text-[#6ea92d]'
        : 'text-[11px] uppercase tracking-[0.22em] text-[#7df29a]'}
      style={{ fontFamily: 'var(--font-pixel)' }}
    >
      {children}
    </div>
  )
}

function ImageFrame({ src, alt }) {
  return (
    <div className="relative overflow-hidden rounded-[28px] border border-white/10 bg-white/[0.04] p-4 shadow-[0_30px_70px_rgba(0,0,0,0.35)]">
      <img
        src={src}
        alt={alt}
        className="aspect-[16/10] w-full rounded-[22px] object-cover"
        loading="lazy"
      />
    </div>
  )
}

function InstallCard({ title, eyebrow, icon: Icon, buttons, note }) {
  return (
    <article className="flex min-h-[280px] flex-col rounded-[22px] border border-black/10 bg-[#f5f4ef] p-5 text-[#171717] shadow-[0_22px_60px_rgba(0,0,0,0.18)]">
      <div className="flex items-center gap-3">
        <div className="rounded-xl bg-white p-2 shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06)]">
          <Icon className="h-5 w-5 text-[#292929]" />
        </div>
        <div>
          <SectionLabel light>{eyebrow}</SectionLabel>
          <h2 className="mt-1 text-[1.65rem] font-semibold leading-none text-[#1d1d1d]">
            {title}
          </h2>
        </div>
      </div>

      <div className="mt-6 space-y-3">
        {buttons.map((button) => (
          <a
            key={button.label}
            href={button.href}
            target="_blank"
            rel="noreferrer"
            className="flex items-center justify-center gap-2 rounded-lg bg-[#8bc34a] px-4 py-3 text-sm font-semibold text-white transition hover:bg-[#79b03a]"
          >
            <Download className="h-4 w-4" />
            <span>{button.label}</span>
          </a>
        ))}
      </div>

      <div className="mt-auto pt-8 text-sm leading-6 text-black/55">{note}</div>
    </article>
  )
}

function FeatureSection({
  eyebrow,
  title,
  description,
  bullets,
  image,
  alt,
  reverse = false,
}) {
  return (
    <section className="grid items-center gap-8 lg:grid-cols-2 lg:gap-12">
      <div className={reverse ? 'lg:order-2' : ''}>
        <SectionLabel>{eyebrow}</SectionLabel>
        <h3 className="mt-4 text-3xl font-semibold leading-tight text-white md:text-4xl">
          {title}
        </h3>
        <p className="mt-4 max-w-xl text-base leading-8 text-white/68 md:text-lg">
          {description}
        </p>
        <ul className="mt-6 space-y-3">
          {bullets.map((bullet) => (
            <li
              key={bullet}
              className="flex items-start gap-3 text-sm leading-7 text-white/70"
            >
              <BadgeCheck className="mt-1 h-4 w-4 shrink-0 text-[#7df29a]" />
              <span>{bullet}</span>
            </li>
          ))}
        </ul>
      </div>

      <div className={reverse ? 'lg:order-1' : ''}>
        <ImageFrame src={image} alt={alt} />
      </div>
    </section>
  )
}

function App() {
  const [releaseState, setReleaseState] = useState({
    status: 'loading',
    release: null,
    error: '',
  })

  useEffect(() => {
    const controller = new AbortController()

    async function loadRelease() {
      try {
        const response = await fetch(RELEASES_API_URL, {
          signal: controller.signal,
          headers: {
            Accept: 'application/vnd.github+json',
          },
        })

        if (!response.ok) {
          throw new Error(`GitHub returned ${response.status}`)
        }

        const release = await response.json()

        setReleaseState({
          status: 'ready',
          release,
          error: '',
        })
      } catch (error) {
        if (controller.signal.aborted) {
          return
        }

        setReleaseState({
          status: 'error',
          release: null,
          error: error?.message ?? 'Could not load release data',
        })
      }
    }

    loadRelease()

    return () => controller.abort()
  }, [])

  const releaseAssets = useMemo(
    () =>
      Array.isArray(releaseState.release?.assets) ? releaseState.release.assets : [],
    [releaseState.release],
  )

  const latestVersion = releaseState.release?.tag_name ?? 'Latest release'
  const latestPublishedAt = formatDate(releaseState.release?.published_at)

  const installCards = [
    {
      title: 'Windows',
      eyebrow: 'DESKTOP',
      icon: Monitor,
      note:
        'Pick EXE for the quickest install, or MSI if you want the more standard Windows installer flow.',
      buttons: [
        {
          label: 'Download EXE',
          href:
            findAsset(releaseAssets, '.exe')?.browser_download_url ??
            RELEASES_PAGE_URL,
        },
        {
          label: 'Download MSI',
          href:
            findAsset(releaseAssets, '.msi')?.browser_download_url ??
            RELEASES_PAGE_URL,
        },
      ],
    },
    {
      title: 'Linux',
      eyebrow: 'PACKAGE',
      icon: TerminalSquare,
      note:
        'Use DEB for Debian or Ubuntu, AppImage for a portable build, or tar.gz if you want a manual archive.',
      buttons: [
        {
          label: 'Download DEB',
          href:
            findAsset(releaseAssets, '.deb')?.browser_download_url ??
            RELEASES_PAGE_URL,
        },
        {
          label: 'Download tar.gz',
          href:
            findAsset(releaseAssets, '.tar.gz')?.browser_download_url ??
            RELEASES_PAGE_URL,
        },
        {
          label: 'Download AppImage',
          href:
            findAsset(releaseAssets, '.appimage')?.browser_download_url ??
            RELEASES_PAGE_URL,
        },
      ],
    },
    {
      title: 'GitHub',
      eyebrow: 'RELEASES',
      icon: FolderArchive,
      note:
        'Open the latest release page for every asset, older versions, checksums, and the source repository.',
      buttons: [
        {
          label: 'Latest Release',
          href: RELEASES_PAGE_URL,
        },
        {
          label: 'Source Code',
          href: REPOSITORY_URL,
        },
      ],
    },
  ]

  const quickStats = [
    {
      icon: Sparkles,
      label: 'Latest build',
      value:
        releaseState.status === 'ready'
          ? latestVersion
          : releaseState.status === 'error'
            ? 'Release page fallback'
            : 'Checking latest release',
    },
    {
      icon: Download,
      label: 'Install formats',
      value: 'EXE, MSI, DEB, tar.gz, AppImage',
    },
    {
      icon: Gamepad2,
      label: 'Practice presets',
      value: 'HQ, Brutal, Wesley',
    },
    {
      icon: Wrench,
      label: 'Core tools',
      value: 'Versions, mods, configs, updates',
    },
  ]

  return (
    <div className="relative min-h-screen overflow-hidden text-white">
      <div className="pointer-events-none absolute left-[-6rem] top-[-3rem] h-72 w-72 rounded-full bg-[#ff6a3d]/20 blur-[110px]" />
      <div className="pointer-events-none absolute right-[-4rem] top-40 h-80 w-80 rounded-full bg-[#7df29a]/14 blur-[130px]" />

      <header className="sticky top-0 z-40 border-b border-white/8 bg-[#090b10]/80 backdrop-blur-xl">
        <div className="mx-auto flex max-w-7xl items-center justify-between px-6 py-4 md:px-8">
          <a href="#top" className="flex items-center gap-3">
            <img
              src="/logo.svg"
              alt="HQ Launcher logo"
              className="h-11 w-11 rounded-2xl border border-white/10 bg-white/5 p-2"
            />
            <div>
              <div
                className="text-[11px] uppercase tracking-[0.22em] text-[#7df29a]"
                style={{ fontFamily: 'var(--font-pixel)' }}
              >
                HQ LAUNCHER
              </div>
              <div className="mt-1 text-sm text-white/55">
                Install page and launcher overview
              </div>
            </div>
          </a>

          <nav className="hidden items-center gap-6 text-sm text-white/65 md:flex">
            <a className="transition hover:text-white" href="#install">
              Install
            </a>
            <a className="transition hover:text-white" href="#overview">
              Overview
            </a>
            <a
              className="transition hover:text-white"
              href={REPOSITORY_URL}
              target="_blank"
              rel="noreferrer"
            >
              GitHub
            </a>
          </nav>
        </div>
      </header>

      <main
        id="top"
        className="mx-auto flex max-w-7xl flex-col gap-24 px-6 pb-20 pt-10 md:gap-28 md:px-8 md:pt-14"
      >
        <section id="install" className="scroll-mt-24 space-y-8">
          <div className="max-w-4xl">
            <div className="inline-flex items-center gap-3 rounded-full border border-white/10 bg-white/[0.04] px-4 py-2 text-sm text-white/65 shadow-[0_12px_36px_rgba(0,0,0,0.22)]">
              {releaseState.status === 'loading' ? (
                <LoaderCircle className="h-4 w-4 animate-spin text-[#ffb36a]" />
              ) : (
                <Sparkles className="h-4 w-4 text-[#ffb36a]" />
              )}
              <span>
                {releaseState.status === 'ready'
                  ? `${latestVersion}${latestPublishedAt ? ` · ${latestPublishedAt}` : ''}`
                  : releaseState.status === 'error'
                    ? `Latest release lookup unavailable · ${releaseState.error}`
                    : 'Checking the latest GitHub release'}
              </span>
            </div>

            <SectionLabel>
              INSTALL HQ LAUNCHER
            </SectionLabel>
            <h1 className="mt-4 max-w-4xl text-5xl font-semibold leading-[0.95] text-white md:text-7xl">
              Pick your platform first, then scroll down for the launcher tour.
            </h1>
            <p className="mt-6 max-w-2xl text-lg leading-8 text-white/68">
              Windows installs are available as EXE or MSI. Linux installs are
              available as DEB, tar.gz, or AppImage. The cards below follow the
              layout you asked for, while the sections underneath are ready for
              your real screenshots.
            </p>
          </div>

          <div className="grid gap-5 md:grid-cols-2 lg:grid-cols-3">
            {installCards.map((card) => (
              <InstallCard key={card.title} {...card} />
            ))}
          </div>

          <div className="flex flex-wrap gap-3">
            {highlightItems.map((item) => {
              const Icon = item.icon

              return (
                <div
                  key={item.label}
                  className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/[0.04] px-4 py-2 text-sm text-white/72"
                >
                  <Icon className="h-4 w-4 text-[#7df29a]" />
                  <span>{item.label}</span>
                </div>
              )
            })}
          </div>
        </section>

        <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {quickStats.map((item) => {
            const Icon = item.icon

            return (
              <div
                key={item.label}
                className="rounded-[28px] border border-white/10 bg-white/[0.04] p-5 shadow-[0_20px_60px_rgba(0,0,0,0.28)]"
              >
                <div className="flex items-center gap-3 text-sm text-white/55">
                  <Icon className="h-4 w-4 text-[#7df29a]" />
                  <span>{item.label}</span>
                </div>
                <div className="mt-4 text-xl font-semibold text-white">
                  {item.value}
                </div>
              </div>
            )
          })}
        </section>

        <section id="overview" className="scroll-mt-24 space-y-16 md:space-y-24">
          <div className="max-w-3xl">
            <SectionLabel>LAUNCHER OVERVIEW</SectionLabel>
            <h2 className="mt-4 text-4xl font-semibold leading-tight text-white md:text-5xl">
              Placeholder shots are now in the exact spots where your real
              launcher photos should go.
            </h2>
            <p className="mt-5 text-lg leading-8 text-white/64">
              I replaced the improvised mock visuals with simple local dummy
              images so you can swap them later without touching the layout.
            </p>
          </div>

          {featureSections.map((section, index) => (
            <FeatureSection
              key={section.title}
              {...section}
              reverse={index % 2 === 1}
            />
          ))}
        </section>

        <section className="relative overflow-hidden rounded-[36px] border border-white/10 bg-white/[0.04] p-8 shadow-[0_32px_80px_rgba(0,0,0,0.45)] backdrop-blur md:p-10">
          <div className="pointer-events-none absolute inset-y-0 right-[-8rem] my-auto h-56 w-56 rounded-full bg-[#ff6a3d]/18 blur-[110px]" />

          <div className="relative flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
            <div className="max-w-2xl">
              <SectionLabel>REPLACE PLACEHOLDERS</SectionLabel>
              <h2 className="mt-4 text-3xl font-semibold leading-tight text-white md:text-4xl">
                Swap the dummy shots with real screenshots whenever you are
                ready.
              </h2>
              <p className="mt-4 text-base leading-8 text-white/64 md:text-lg">
                Replace the files in <code className="rounded bg-white/10 px-2 py-1 text-sm text-white">public/placeholders</code> and the landing page will use your
                updated images immediately.
              </p>
            </div>

            <div className="flex flex-wrap gap-3">
              <a
                href={RELEASES_PAGE_URL}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-5 py-3 text-sm font-medium text-white transition hover:bg-white/10"
              >
                <Download className="h-4 w-4" />
                <span>Latest releases</span>
              </a>
              <a
                href={REPOSITORY_URL}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-2 rounded-full border border-white/10 px-5 py-3 text-sm font-medium text-white/80 transition hover:bg-white/5 hover:text-white"
              >
                <Github className="h-4 w-4" />
                <span>Source on GitHub</span>
              </a>
              <a
                href="#install"
                className="inline-flex items-center gap-2 rounded-full border border-white/10 px-5 py-3 text-sm font-medium text-white/80 transition hover:bg-white/5 hover:text-white"
              >
                <ArrowUpRight className="h-4 w-4" />
                <span>Back to install</span>
              </a>
            </div>
          </div>
        </section>
      </main>
    </div>
  )
}

export default App
