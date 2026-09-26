import { ExternalLink, PageHeader, Panel } from "../components/common";
import { PROJECT, REPOSITORY, SUPPORT, displayUrl, type ProjectLink } from "../lib/links";
import { href } from "../lib/router";

function LinkList({ links }: { links: ProjectLink[] }) {
  return (
    <ul className="list">
      {links.map((link) => (
        <li key={link.url}>
          <div>
            <strong>{link.label}</strong>
            <div className="muted">{link.description}</div>
          </div>
          <ExternalLink href={link.url}>{displayUrl(link.url)}</ExternalLink>
        </li>
      ))}
    </ul>
  );
}

export function About() {
  return (
    <>
      <PageHeader title="About & support">
        RepoDNA is free, open-source repository intelligence and code archaeology.
      </PageHeader>
      <Panel title="RepoDNA">
        <div className="about">
          <img src="./favicon.svg" alt="" width={48} height={48} />
          <div>
            <strong>RepoDNA v{__REPODNA_VERSION__}</strong>
            <div className="muted">
              Evidence-backed analysis of a repository's structure, architecture, history, and
              health, on your own machine and without telemetry.
            </div>
          </div>
        </div>
        <p>
          Made by the Sanskar and developed in the open with its contributors. Licensed under the
          Apache License 2.0.
        </p>
      </Panel>
      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel
          title="Support RepoDNA"
          description="RepoDNA is free for everyone. If it helps you, you can support its development:"
        >
          <LinkList links={SUPPORT} />
          <p className="muted">
            Starring the <ExternalLink href={REPOSITORY}>repository</ExternalLink>, reporting
            problems, and contributing code or documentation help as well.
          </p>
        </Panel>
        <Panel title="Legal">
          <ul className="list">
            <li>
              <div>
                <strong>
                  <a href={href("/privacy")}>Privacy Policy</a>
                </strong>
                <div className="muted">
                  RepoDNA collects no personal information and sends no telemetry.
                </div>
              </div>
            </li>
            <li>
              <div>
                <strong>
                  <a href={href("/terms")}>Terms of Use</a>
                </strong>
                <div className="muted">The terms for using RepoDNA and its web version.</div>
              </div>
            </li>
            <li>
              <div>
                <strong>
                  <a href={href("/licenses")}>Licenses</a>
                </strong>
                <div className="muted">
                  RepoDNA&apos;s license and the third-party software it includes.
                </div>
              </div>
            </li>
            <li>
              <div>
                <strong>
                  <ExternalLink href={`${REPOSITORY}/blob/main/SECURITY.md`}>
                    Security policy
                  </ExternalLink>
                </strong>
                <div className="muted">How to report a vulnerability privately.</div>
              </div>
            </li>
            <li>
              <div>
                <strong>
                  <ExternalLink href={`${REPOSITORY}/blob/main/CODE_OF_CONDUCT.md`}>
                    Code of conduct
                  </ExternalLink>
                </strong>
                <div className="muted">How we work together in the RepoDNA community.</div>
              </div>
            </li>
          </ul>
        </Panel>
      </div>
      <Panel title="Links">
        <LinkList links={PROJECT} />
      </Panel>
    </>
  );
}
