import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

/**
 * One hand-written sidebar, ordered the way an engineer adopts the tool: what it is and how to run
 * it, the model behind it, task-oriented guides, the reference tables, worked examples with real
 * output, and an honest account of what is and is not built.
 */
const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Start here',
      collapsed: false,
      items: ['index', 'getting-started'],
    },
    {
      type: 'category',
      label: 'Concepts',
      collapsed: false,
      items: [
        'concepts/overview',
        'concepts/aep',
        'concepts/evidence',
        'concepts/ess',
        'concepts/design-principles',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      collapsed: false,
      items: [
        'guides/govern-a-task',
        'guides/write-a-principle',
        'guides/integrate-a-harness',
        'guides/write-a-specification',
        'guides/generate-artifacts',
        'guides/verify-conformance',
        'guides/track-change',
        'guides/synthesize',
        'guides/check-infrastructure',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      collapsed: false,
      items: [
        'reference/cli',
        'reference/documents',
        'reference/vocabulary',
        'reference/glossary',
      ],
    },
    {
      type: 'category',
      label: 'Examples',
      collapsed: false,
      items: ['examples/specification-to-contracts', 'examples/governed-task'],
    },
    {
      type: 'category',
      label: 'Project status',
      collapsed: false,
      items: ['status/where-this-stands', 'status/limitations', 'status/roadmap'],
    },
  ],
};

export default sidebars;
