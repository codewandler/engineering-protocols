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
        // No blog. There is nothing to announce yet, and an empty blog is a dead link on a navbar.
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/docusaurus-social-card.jpg',
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
          to: '/docs/in-practice/a-specification-and-its-contracts',
          label: 'See it work',
          position: 'left',
        },
        {
          to: '/docs/status/where-this-stands',
          label: 'Status',
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
          title: 'Start here',
          items: [
            {label: 'What this is', to: '/docs'},
            {label: 'Why agents change this', to: '/docs/why-agents-change-this'},
            {label: 'The two halves', to: '/docs/two-halves'},
            {label: 'What this is not', to: '/docs/deliberately-not'},
          ],
        },
        {
          title: 'In practice',
          items: [
            {
              label: 'A specification and its contracts',
              to: '/docs/in-practice/a-specification-and-its-contracts',
            },
            {label: 'What a refusal looks like', to: '/docs/in-practice/refusals'},
            {label: 'A governed task', to: '/docs/in-practice/a-governed-task'},
          ],
        },
        {
          title: 'Where this stands',
          items: [
            {label: 'Status', to: '/docs/status/where-this-stands'},
            {label: 'What you still have to trust', to: '/docs/status/what-you-have-to-trust'},
            {label: 'Proposed, not accepted', to: '/docs/status/proposed-not-accepted'},
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
