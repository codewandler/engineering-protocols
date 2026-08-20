import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

/**
 * One hand-written sidebar. The order is the order a reader who arrives cold should meet things in:
 * the problem, why it is a problem now, the two halves, the commitments — then the artifacts that
 * make the claims checkable, then what is honestly not built.
 */
const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Start here',
      collapsed: false,
      items: [
        'index',
        'why-agents-change-this',
        'two-halves',
        'pillars',
        'deliberately-not',
      ],
    },
    {
      type: 'category',
      label: 'In practice',
      collapsed: false,
      items: [
        'in-practice/a-specification-and-its-contracts',
        'in-practice/refusals',
        'in-practice/a-governed-task',
        'in-practice/the-join',
      ],
    },
    {
      type: 'category',
      label: 'Where this stands',
      collapsed: false,
      items: [
        'status/where-this-stands',
        'status/what-you-have-to-trust',
        'status/proposed-not-accepted',
      ],
    },
  ],
};

export default sidebars;
