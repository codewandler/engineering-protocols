import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';

import styles from './styles.module.css';

/**
 * The play button that opens the lab, for use from a documentation page.
 *
 * WHY THIS IS A COMPONENT AND NOT RAW HTML, which is what it looks like it should be:
 *
 * Docusaurus compiles `.md` through MDX — `markdown.format` defaults to `mdx`, and this site does
 * not override it — so HTML written into a Markdown file *does* render, as JSX. It renders wrong,
 * though, in a way nothing catches. MDX routes only the elements it generates from Markdown syntax
 * through the theme's component map; an `<a>` written literally in the file compiles to a literal
 * `<a>`, which means it never reaches `@docusaurus/Link`. `Link` is what prepends `baseUrl`, and
 * this site is served under `/engineering-protocols/`, so a hand-written `<a href="/lab">` ships as
 * `href="/lab"` and 404s in production. It also escapes `onBrokenLinks: 'throw'`, which collects
 * links from `Link` rather than from anchors in the emitted HTML — verified: the raw version built
 * green and emitted `<a class=lab-launch href=/lab>`.
 *
 * So the button is one import in the Markdown file and a `<Link>` here. Same markup, `baseUrl`
 * applied, and the target now in front of the broken-link check.
 */
export default function LabLaunch(): ReactNode {
  return (
    <Link className={styles.launch} to="/lab">
      <span className={styles.play} aria-hidden="true">
        ▶
      </span>
      <span className={styles.text}>
        <strong>Open in the lab</strong>
        <span>Step this file, the IR it compiles to and a run of it, side by side.</span>
      </span>
      <span className={styles.tag}>draft</span>
    </Link>
  );
}
