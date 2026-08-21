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

/**
 * Every section below is one panel, drawn the way the diagrams draw a panel: a header strip
 * carrying an ordinal, a label and (where there is one) a chip, then the body. The ordinals are
 * the page's argument in order — the problem, the shape of the answer, the claim, the status —
 * and they are the only thing on the page that is not either existing copy or a file name.
 */
type PanelProps = {
  ordinal: string;
  label: string;
  title: string;
  chip?: ReactNode;
  alt?: boolean;
  children: ReactNode;
};

function PanelSection({ordinal, label, title, chip, alt, children}: PanelProps) {
  return (
    <section className={clsx(styles.section, alt && styles.sectionAlt)}>
      <div className={styles.sectionInner}>
        <div className={styles.panel}>
          <div className={styles.panelHeader}>
            <div className={styles.panelEyebrow}>
              <span className={styles.panelOrdinal}>{ordinal}</span>
              <span>{label}</span>
            </div>
            {chip ? <span className={styles.chip}>{chip}</span> : null}
          </div>
          <Heading as="h2" className={styles.panelTitle}>
            {title}
          </Heading>
          <div className={styles.panelBody}>{children}</div>
        </div>
      </div>
    </section>
  );
}

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero', styles.heroBanner)}>
      <div className={styles.heroInner}>
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
            to="/docs/examples/specification-to-contracts">
            See it work
          </Link>
        </div>
      </div>
    </header>
  );
}

function TheProblem() {
  return (
    <PanelSection ordinal="01" label="The problem" title="Two documents nobody can check">
      <p>
        Every engineering organisation runs on two pieces of prose: the one that says how we work,
        and the one that says what we are building. Both are read by people who then go and do
        something else.
      </p>
      <CodeBlock language="text">
        {`"Follow TDD, don't break the API, get approval before touching production."
        → a wiki page nobody consults during the work

"The billing service issues invoices; a paid invoice cannot be cancelled."
        → a ticket, an out-of-date API doc, and an argument six months later`}
      </CodeBlock>
      <p>
        A person who ignores the wiki page can be asked why. An agent given the same page in a
        prompt produces something that <em>reads</em> as though it followed it, at whatever scale
        you run it. Prose instructions do not fail loudly; they fail silently and plausibly — and
        reviewing the output does not scale to the volume agents produce.
      </p>
      <p className={styles.panelMore}>
        <Link to="/docs">Why this changes once agents write the code →</Link>
      </p>
    </PanelSection>
  );
}

function TheClaim() {
  return (
    <PanelSection
      ordinal="03"
      label="The claim"
      title="The specification is not a document beside the contract"
      alt>
      <p>
        On the left, part of one command in a specification. On the right, part of the OpenAPI
        document generated from it — the two outcomes became two status codes, and CI fails if the
        committed output stops matching the source.
      </p>
      <div className={styles.compare}>
        <div className={styles.compareSide}>
          <CodeBlock language="yaml" title="examples/billing/domains/invoice.yaml">
            {SPECIFICATION}
          </CodeBlock>
        </div>
        <div className={styles.compareArrow} aria-hidden="true">
          <span>→</span>
        </div>
        <div className={styles.compareSide}>
          <CodeBlock language="yaml" title="generated/openapi/invoice-service.yaml">
            {GENERATED}
          </CodeBlock>
        </div>
      </div>
      <p>
        A command that can be refused has to say so. A specification recording only the happy branch
        generates a suite that never checks the branch where the money does not move.
      </p>
      <p className={styles.panelMore}>
        <Link to="/docs/examples/specification-to-contracts">
          The whole example, including the JSON Schema and the AsyncAPI →
        </Link>
      </p>
    </PanelSection>
  );
}

function HonestStatus() {
  return (
    <PanelSection
      ordinal="04"
      label="Status"
      title="What is built, and what is not"
      chip={<code>0.7.1-infra-waves-1-4</code>}>
      <div className={styles.ledger}>
        <p className={styles.ledgerBuilt}>
          The protocol is implemented and gated: <strong>106 suites and 1811 tests</strong>, with 0
          clippy warnings and 0 rustdoc warnings, as of the tag <code>0.7.1-infra-waves-1-4</code>.
          A specification compiles into documentation, JSON Schema, OpenAPI 3.1 and AsyncAPI 3.0;
          generates its own conformance suite; and synthesises the structural part of its own
          implementation in three targets — the same specification runs as a Rust and a Go
          application, both started and held to one behaviour in every CI run. All of it is
          drift-checked.
        </p>
        <p className={styles.ledgerNot}>
          It does <strong>not</strong> generate behaviour — every algorithm is a typed obligation
          someone still has to implement. There is no durable backend. No team has been governed by
          this yet. And one thing you still have to trust: nothing binds a verifier&apos;s identity
          to the evidence it submits.
        </p>
      </div>
      <p className={styles.panelMore}>
        <Link to="/docs/status/where-this-stands">Where this stands →</Link>
        {' · '}
        <Link to="/docs/status/limitations">Limitations and trust assumptions →</Link>
      </p>
    </PanelSection>
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
        <TheProblem />
        <HomepageFeatures />
        <TheClaim />
        <HonestStatus />
      </main>
    </Layout>
  );
}
