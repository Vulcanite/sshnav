import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import styles from './index.module.css';

const asciiLogo = String.raw` ____  ____  _   _  _   _    _    __     __
/ ___|/ ___|| | | || \ | |  / \   \ \   / /
\___ \\___ \| |_| ||  \| | / _ \   \ \ / /
 ___) |___) |  _  || |\  |/ ___ \   \ V /
|____/|____/|_| |_||_| \_/_/   \_\   \_/`;

const features = [
  ['01', 'Find', 'Fuzzy-search aliases, groups, users, hosts, and tags from one keyboard-first picker.'],
  ['02', 'Connect', 'Launch native OpenSSH with saved ports, forwards, options, jump chains, and encrypted identities.'],
  ['03', 'Transfer', 'Send and receive files through native scp or resumable rsync without rebuilding connection arguments by hand.'],
];

function Homepage(): ReactNode {
  return (
    <Layout title="Local SSH navigation" description="A fast local SSH inventory navigator and launcher.">
      <main>
        <section className={styles.hero}>
          <div className={styles.coordinate}>LOCAL / SQLITE / OPENSSH</div>
          <div className={styles.heroGrid}>
            <div className={styles.heroCopy}>
              <span className={styles.eyebrow}>SSH inventory under control</span>
              <Heading as="h1">Stop remembering hosts.<br />Start navigating them.</Heading>
              <p>
                sshnav keeps aliases, jump routes, keys, and transfer settings in one local inventory—then
                hands the connection to the OpenSSH tools you already trust.
              </p>
              <div className={styles.actions}>
                <Link className={clsx('button', styles.primaryAction)} to="/docs/getting-started/quickstart">
                  Start in 3 minutes <span aria-hidden="true">→</span>
                </Link>
                <Link className={clsx('button', styles.secondaryAction)} to="/docs/reference/cli">
                  Read the CLI
                </Link>
              </div>
            </div>
            <div className={styles.terminal} aria-label="sshnav terminal preview">
              <div className={styles.terminalBar}>
                <span>sshnav — prod</span>
                <span className={styles.status}>● READY</span>
              </div>
              <pre className={styles.ascii} aria-hidden="true">{asciiLogo}</pre>
              <div className={styles.commandLine}><span>$</span> sshnav send prod ./release /srv/app -r</div>
              <div className={styles.routeLine}>
                <span>local</span><i /> <span>bastion</span><i /> <strong>prod</strong>
              </div>
            </div>
          </div>
          <div className={styles.heroIndex}>[ 0001 — 0022 ]</div>
        </section>

        <section className={styles.featureSection} aria-labelledby="workflow-heading">
          <div className={styles.sectionHeading}>
            <span>OPERATING MODEL</span>
            <Heading as="h2" id="workflow-heading">One inventory. Three moves.</Heading>
          </div>
          <div className={styles.featureGrid}>
            {features.map(([index, title, body]) => (
              <article className={styles.feature} key={index}>
                <span className={styles.featureIndex}>{index}</span>
                <Heading as="h3">{title}</Heading>
                <p>{body}</p>
              </article>
            ))}
          </div>
        </section>

        <section className={styles.commandSection} aria-labelledby="command-heading">
          <div>
            <span className={styles.eyebrow}>Native tools, composed safely</span>
            <Heading as="h2" id="command-heading">No shell strings.<br />No mystery state.</Heading>
          </div>
          <div className={styles.commandStack}>
            <code><b>CONNECT</b> sshnav connect production</code>
            <code><b>SEND</b> sshnav send production ./build /srv/app -r</code>
            <code><b>RECEIVE</b> sshnav receive production /var/log/app.log</code>
            <code><b>TRACE</b> sshnav doctor production</code>
          </div>
        </section>

        <section className={styles.finalCta}>
          <span>READY WHEN YOUR NEXT HOST IS.</span>
          <Link to="/docs/getting-started/installation">Install sshnav <span aria-hidden="true">↗</span></Link>
        </section>
      </main>
    </Layout>
  );
}

export default Homepage;
