import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import CodeBlock from '@theme/CodeBlock';
import HomepageFeatures from '@site/src/components/HomepageFeatures';
import Heading from '@theme/Heading';

import styles from './index.module.css';

// Copied from examples/billing/domains/invoice.yaml. Not paraphrased: the point of the pairing
// below is that the right-hand side is what this exact text produced.
const SPECIFICATION = `outcomes:
  - name: accepted
    when: amount.amount > 0
    creates: billing.invoice.Invoice
    emits:
      - billing.invoice.InvoiceCreated

  - name: rejected
    error: billing.invoice.InvalidAmount`;

// Copied from generated/openapi/invoice-service.yaml, which cargo xtask generate --check keeps in
// step with the specification above.
const GENERATED = `responses:
  '202':
    description: 'Outcome \`accepted\`: the branch the
      specification declares for this input.'
    ...
  '422':
    description: 'Outcome \`rejected\`: the request was
      understood and refused on domain grounds.'
    ...`;

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero', styles.heroBanner)}>
      <div className="container">
        <Heading as="h1" className="hero__title">
          {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link className="button button--primary button--lg" to="/docs">
            What this is
          </Link>
          <Link
            className="button button--secondary button--lg"
            to="/docs/in-practice/a-specification-and-its-contracts">
            See it work
          </Link>
        </div>
      </div>
    </header>
  );
}

function TheProblem() {
  return (
    <section className={styles.section}>
      <div className="container">
        <div className="row">
          <div className="col col--8 col--offset-2">
            <Heading as="h2">Two documents nobody can check</Heading>
            <p>
              Every engineering organisation runs on two pieces of prose: the one that says how we
              work, and the one that says what we are building. Both are read by people who then go
              and do something else.
            </p>
            <CodeBlock language="text">
              {`"Follow TDD, don't break the API, get approval before touching production."
        → a wiki page nobody consults during the work

"The billing service issues invoices; a paid invoice cannot be cancelled."
        → a ticket, an out-of-date API doc, and an argument six months later`}
            </CodeBlock>
            <p>
              A person who ignores the wiki page can be asked why. An agent given the same page in a
              prompt produces something that <em>reads</em> as though it followed it, at whatever
              scale you run it. Prose instructions do not fail loudly; they fail silently and
              plausibly — and reviewing the output does not scale to the volume agents produce.
            </p>
            <p>
              <Link to="/docs/why-agents-change-this">
                Why this changes once agents write the code →
              </Link>
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function TheClaim() {
  return (
    <section className={clsx(styles.section, styles.sectionAlt)}>
      <div className="container">
        <Heading as="h2">The specification is not a document beside the contract</Heading>
        <p>
          On the left, part of one command in a specification. On the right, part of the OpenAPI
          document generated from it — the two outcomes became two status codes, and CI fails if the
          committed output stops matching the source.
        </p>
        <div className="row">
          <div className="col col--6">
            <CodeBlock language="yaml" title="examples/billing/domains/invoice.yaml">
              {SPECIFICATION}
            </CodeBlock>
          </div>
          <div className="col col--6">
            <CodeBlock language="yaml" title="generated/openapi/invoice-service.yaml">
              {GENERATED}
            </CodeBlock>
          </div>
        </div>
        <p>
          A command that can be refused has to say so. A specification recording only the happy
          branch generates a suite that never checks the branch where the money does not move.
        </p>
        <p>
          <Link to="/docs/in-practice/a-specification-and-its-contracts">
            The whole example, including the JSON Schema and the AsyncAPI →
          </Link>
        </p>
      </div>
    </section>
  );
}

function HonestStatus() {
  return (
    <section className={styles.section}>
      <div className="container">
        <div className="row">
          <div className="col col--8 col--offset-2">
            <Heading as="h2">What is built, and what is not</Heading>
            <p>
              The protocol is implemented and gated: <strong>41 suites and 953 tests</strong>, with 0
              clippy warnings and 0 rustdoc warnings, as of the tag <code>0.3.2-ess-wave-3</code>. A
              specification compiles into documentation, JSON Schema, OpenAPI 3.1 and AsyncAPI 3.0,
              and the output is drift-checked.
            </p>
            <p>
              It does <strong>not</strong> yet generate tests or code — that is the next two waves.
              There is no durable backend. No team has been governed by this yet. And one thing you
              still have to trust: nothing binds a verifier&apos;s identity to the evidence it
              submits.
            </p>
            <p>
              <Link to="/docs/status/where-this-stands">Where this stands →</Link>
              {' · '}
              <Link to="/docs/status/what-you-have-to-trust">
                What you still have to trust →
              </Link>
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title="Agentic engineering under machine-checkable constraints"
      description={siteConfig.tagline as string}>
      <HomepageHeader />
      <main>
        <HomepageFeatures />
        <TheProblem />
        <TheClaim />
        <HonestStatus />
      </main>
    </Layout>
  );
}
