import type {PrismTheme} from 'prism-react-renderer';
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

// Syntax highlighting, in the site's own palette rather than a stock theme.
//
// The code block is the diagrams' panel: `#161d24` on `#0f1418`, a `#2b3540` hairline drawn in CSS.
// Docusaurus writes the theme's plain colours onto the block as inline custom properties, so a
// stylesheet cannot reach them — the background has to be set here or the block will not match the
// panels beside it. The token hues are the palette's own: green for the keys a specification is
// mostly made of, blue for values and types, amber for variables, one red for keywords, and the
// muted grey for comments and punctuation. No purple: it is not a colour this project uses.
const MONO_STACK =
  "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', 'Liberation Mono', monospace";

const darkCodeTheme: PrismTheme = {
  plain: {color: '#d7dde3', backgroundColor: '#161d24'},
  styles: [
    {
      types: ['comment', 'prolog', 'doctype', 'cdata'],
      style: {color: '#8b98a5', fontStyle: 'italic'},
    },
    {types: ['punctuation', 'operator', 'entity'], style: {color: '#8b98a5'}},
    {
      types: ['keyword', 'selector', 'tag', 'deleted', 'important', 'rule'],
      style: {color: '#ff7b72'},
    },
    {types: ['string', 'char', 'attr-value', 'inserted', 'regex'], style: {color: '#a5d6ff'}},
    {
      types: ['number', 'boolean', 'constant', 'symbol', 'class-name', 'function', 'builtin'],
      style: {color: '#79c0ff'},
    },
    {types: ['atrule', 'attr-name', 'property', 'key'], style: {color: '#7ee787'}},
    {types: ['variable', 'parameter'], style: {color: '#ffa657'}},
    {types: ['namespace'], style: {opacity: 0.75}},
  ],
};

const lightCodeTheme: PrismTheme = {
  plain: {color: '#1f2830', backgroundColor: '#f6f8fa'},
  styles: [
    {
      types: ['comment', 'prolog', 'doctype', 'cdata'],
      style: {color: '#5c6570', fontStyle: 'italic'},
    },
    {types: ['punctuation', 'operator', 'entity'], style: {color: '#57636f'}},
    {
      types: ['keyword', 'selector', 'tag', 'deleted', 'important', 'rule'],
      style: {color: '#cf222e'},
    },
    {types: ['string', 'char', 'attr-value', 'inserted', 'regex'], style: {color: '#0a3069'}},
    {
      types: ['number', 'boolean', 'constant', 'symbol', 'class-name', 'function', 'builtin'],
      style: {color: '#0550ae'},
    },
    {types: ['atrule', 'attr-name', 'property', 'key'], style: {color: '#116329'}},
    {types: ['variable', 'parameter'], style: {color: '#953800'}},
    {types: ['namespace'], style: {opacity: 0.75}},
  ],
};

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
      // The mark is the evidence panel from the social card, reduced to one glyph. It carries its
      // own dark fill, so one file reads on both themes and no `srcDark` variant is needed.
      // `static/img/favicon.ico` is rasterized from it — see the note in `static/img/mark.svg`.
      logo: {
        alt: 'engineering-protocols',
        src: 'img/mark.svg',
        width: 26,
        height: 26,
      },
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
          // Icon and word: `custom.css` masks the GitHub mark in front of the label with
          // `currentColor`, and suppresses the external-link arrow Docusaurus would add beside it.
          className: 'navbar-github-link',
          'aria-label': 'GitHub repository',
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
      // The mark signs the page, and the claim under it is the site's tagline verbatim — the same
      // sentence the hero opens with, closing the page where it started.
      logo: {
        alt: 'engineering-protocols',
        src: 'img/mark.svg',
        href: '/',
        width: 22,
        height: 22,
      },
      copyright:
        '<span class="footer__claim">Constrain agent-written engineering work with rules a program ' +
        'executes, and decide completion from evidence rather than assertion.</span>' +
        'engineering-protocols · Apache-2.0 · built with Docusaurus.',
    },
    prism: {
      theme: lightCodeTheme,
      darkTheme: darkCodeTheme,
      additionalLanguages: ['rust', 'yaml', 'json', 'bash'],
    },
    // The generated diagrams are set in the same typeface as the drawn ones, and light mode uses
    // Mermaid's `neutral` base rather than its default: the default fills every state node
    // lavender, which is a colour this project does not use anywhere else. `neutral` is grey on
    // white, which is what the drawn diagrams are, inverted.
    mermaid: {
      theme: {light: 'neutral', dark: 'dark'},
      options: {
        fontFamily: MONO_STACK,
      },
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
