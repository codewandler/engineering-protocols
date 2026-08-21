import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

// This site is written here, in `website/docs/`, for a reader who has never seen the repository.
// It is not a rendering of the repository's own `docs/` tree: that tree is the internal engineering
// record — design proposals nobody has accepted, reviews cataloguing defects, gate pages — and it is
// written for contributors, not for the public. Nothing in this build reads outside `website/`.
//
// DEPLOYMENT, and the hazard that comes with it: GitHub Pages can be configured to publish from the
// `/docs` folder of the default branch. Do not use that option for this repository. It would publish
// `docs/` — the internal record — verbatim, at the project's public URL. Publish the *built output*
// of this directory to the `gh-pages` branch (or upload it as an Actions Pages artifact) instead.

const config: Config = {
  title: 'Engineering Protocols',
  tagline:
    'Constrain agent-written engineering work with rules a program executes, and decide completion from evidence rather than assertion.',
  favicon: 'img/favicon.ico',

  // Future flags, see https://docusaurus.io/docs/api/docusaurus-config#future
  future: {
    v4: true, // Improve compatibility with the upcoming Docusaurus v4
  },

  // Published at https://codewandler.github.io/engineering-protocols/
  url: 'https://codewandler.github.io',
  baseUrl: '/engineering-protocols/',

  // GitHub Pages deployment: `npm run build` here, publish `website/build/` to `gh-pages`.
  organizationName: 'codewandler',
  projectName: 'engineering-protocols',
  deploymentBranch: 'gh-pages',
  trailingSlash: false,

  // Left as `throw`. A site whose subject is checkable claims does not ship dead links.
  onBrokenLinks: 'throw',

  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
    // The documentation generator already emits Mermaid — a lifecycle as a `stateDiagram-v2`, the
    // system as a flowchart. Rendering it here means the diagrams on this site are the generated
    // artifacts themselves rather than drawings of them, which is the claim the site is making.
    mermaid: true,
  },

  themes: ['@docusaurus/theme-mermaid'],

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          routeBasePath: 'docs',
          editUrl:
            'https://github.com/codewandler/engineering-protocols/tree/main/website/',
        },
        // The blog is the release record for a reader: one post per release, each a worked
        // tutorial on what the release adds, with real command output rather than claims about it.
        // It stayed `false` until there was a first post, because an empty blog is a dead link.
        blog: {
          routeBasePath: 'releases',
          blogTitle: 'Releases, in practice',
          blogDescription:
            'What each release adds to the tooling, shown on real output rather than described.',
          blogSidebarTitle: 'All releases',
          blogSidebarCount: 'ALL',
          showReadingTime: true,
          // Same policy as links: a post that leaks its full body onto the index page is a defect.
          onUntruncatedBlogPosts: 'throw',
          editUrl:
            'https://github.com/codewandler/engineering-protocols/tree/main/website/',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    // The social/Open Graph preview card. Source of truth is `static/img/social-card.svg`;
    // re-rasterize after editing it: `rsvg-convert -w 1200 -h 630 static/img/social-card.svg
    // -o static/img/social-card.png` (1200x630 is what Slack, X and LinkedIn crop against).
    image: 'img/social-card.png',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Engineering Protocols',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Documentation',
        },
        {
          to: '/docs/examples/specification-to-contracts',
          label: 'See it work',
          position: 'left',
        },
        {
          to: '/docs/status/where-this-stands',
          label: 'Status',
          position: 'left',
        },
        {
          to: '/releases',
          label: 'Releases',
          position: 'left',
        },
        {
          href: 'https://github.com/codewandler/engineering-protocols',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentation',
          items: [
            {label: 'Introduction', to: '/docs'},
            {label: 'Getting started', to: '/docs/getting-started'},
            {label: 'Architecture overview', to: '/docs/concepts/overview'},
            {label: 'Design principles', to: '/docs/concepts/design-principles'},
            {label: 'CLI reference', to: '/docs/reference/cli'},
          ],
        },
        {
          title: 'Examples',
          items: [
            {
              label: 'A specification and its contracts',
              to: '/docs/examples/specification-to-contracts',
            },
            {label: 'A governed task, end to end', to: '/docs/examples/governed-task'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'Status', to: '/docs/status/where-this-stands'},
            {label: 'Limitations and trust assumptions', to: '/docs/status/limitations'},
            {label: 'Roadmap and proposals', to: '/docs/status/roadmap'},
            {label: 'Releases, in practice', to: '/releases'},
            {
              label: 'Source',
              href: 'https://github.com/codewandler/engineering-protocols',
            },
          ],
        },
      ],
      copyright: `engineering-protocols · Apache-2.0 · built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'yaml', 'json', 'bash'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
