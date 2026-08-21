import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  governs: string;
  question: string;
  description: ReactNode;
  href: string;
};

// Two halves, and the join. Every claim here is stated at greater length — with its source — on the
// page it links to.
const FeatureList: FeatureItem[] = [
  {
    title: 'AEP',
    governs: 'how engineering work is performed',
    question: 'Was this built properly?',
    description: (
      <>
        Principles with timed obligations, workflows guarded by evidence, capabilities that default
        to denied, approvals bound to the revision they approved, and an audit trail that records
        refusals as carefully as changes. A harness asks what is owed and what is permitted; the
        answer is deterministic, and it can always say why.
      </>
    ),
    href: '/docs/concepts/aep',
  },
  {
    title: 'ESS',
    governs: 'what software must exist',
    question: 'Is this the thing we meant to build?',
    description: (
      <>
        Domains, entities, commands with outcomes, events, views with declared consistency, state
        machines, components, bindings, topology. From one model come the documentation, the JSON
        Schema, the OpenAPI and the AsyncAPI — derived, not maintained beside it.
      </>
    ),
    href: '/docs/examples/specification-to-contracts',
  },
  {
    title: 'The join',
    governs: 'evidence',
    question: 'Who says it is done?',
    description: (
      <>
        A task can be blocked until something <em>other than the agent</em> proves the implementation
        conforms to its specification. The specification judges the diff, so nobody has to read it
        and guess — and the protocol refuses to call the task done until it has.
      </>
    ),
    href: '/docs/concepts/evidence',
  },
];

function Feature({title, governs, question, description, href}: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className={styles.card}>
        <Heading as="h3" className={styles.cardTitle}>
          <Link to={href}>{title}</Link>
        </Heading>
        <p className={styles.cardGoverns}>{governs}</p>
        <p className={styles.cardQuestion}>{question}</p>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): ReactNode {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
