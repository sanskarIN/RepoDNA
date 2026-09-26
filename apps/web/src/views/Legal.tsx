import { useEffect, useState, type ReactNode } from "react";
import privacySource from "../../../../PRIVACY.md?raw";
import termsSource from "../../../../TERMS.md?raw";
import { ExternalLink, PageHeader, Panel } from "../components/common";
import { IN_APP_DOCUMENTS, REPOSITORY } from "../lib/links";
import { MarkdownDocument } from "../lib/markdown";
import { href } from "../lib/router";

/** Links in the documents: pages of this interface stay here, others open in the browser. */
function documentLink(url: string, children: ReactNode, key: string): ReactNode {
  const route = IN_APP_DOCUMENTS[url];
  if (route) {
    return (
      <a key={key} href={href(route)}>
        {children}
      </a>
    );
  }
  return (
    <ExternalLink key={key} href={url}>
      {children}
    </ExternalLink>
  );
}

export function PrivacyPolicy() {
  return <MarkdownDocument source={privacySource} link={documentLink} />;
}

export function TermsOfUse() {
  return <MarkdownDocument source={termsSource} link={documentLink} />;
}

type Loaded = { state: "loading" } | { state: "loaded"; text: string } | { state: "failed" };

/** Reads a text file published next to the web interface. */
async function fetchText(file: string): Promise<string> {
  const response = await fetch(`./${file}`);
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}`);
  }
  return response.text();
}

/** A text file published next to the web interface, such as the license notices. */
function TextFile({ file, source, label }: { file: string; source: string; label: string }) {
  const [loaded, setLoaded] = useState<Loaded>({ state: "loading" });
  useEffect(() => {
    let current = true;
    fetchText(file).then(
      (text) => {
        if (current) {
          setLoaded({ state: "loaded", text });
        }
      },
      () => {
        if (current) {
          setLoaded({ state: "failed" });
        }
      },
    );
    return () => {
      current = false;
    };
  }, [file]);
  if (loaded.state === "loaded") {
    return (
      <pre className="license-text" tabIndex={0} aria-label={label}>
        {loaded.text}
      </pre>
    );
  }
  return (
    <p className="muted">
      {loaded.state === "loading" ? "Loading…" : "This file could not be loaded here. "}
      {loaded.state === "failed" ? (
        <>
          Read it on GitHub:{" "}
          <ExternalLink href={`${REPOSITORY}/blob/main/${source}`}>{source}</ExternalLink>.
        </>
      ) : null}
    </p>
  );
}

export function Licenses() {
  return (
    <>
      <PageHeader title="Licenses">
        RepoDNA is free and open-source software. Here are its license and the licenses of the
        software it includes.
      </PageHeader>
      <Panel title="RepoDNA">
        <p>
          Copyright 2026 Sanskar. Licensed under the Apache License, Version 2.0: you may use, copy,
          modify, and distribute RepoDNA under its terms. See also the{" "}
          <a href={href("/terms")}>Terms of Use</a> and the{" "}
          <a href={href("/privacy")}>Privacy Policy</a>.
        </p>
        <details>
          <summary>Apache License 2.0</summary>
          <TextFile file="LICENSE.txt" source="LICENSE" label="The Apache License 2.0" />
        </details>
        <details>
          <summary>NOTICE</summary>
          <TextFile file="NOTICE.txt" source="NOTICE" label="RepoDNA's NOTICE file" />
        </details>
      </Panel>
      <Panel
        title="Third-party software"
        description="RepoDNA includes open-source software by others: Rust crates in the command line and the desktop app, npm packages in the web interface, and the DejaVu fonts. Their licenses and notices:"
      >
        <TextFile
          file="THIRD-PARTY-NOTICES.txt"
          source="THIRD-PARTY-NOTICES.txt"
          label="Third-party notices"
        />
      </Panel>
    </>
  );
}
