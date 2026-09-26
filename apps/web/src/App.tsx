import { useEffect, type ReactNode } from "react";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Shell } from "./components/Shell";
import { NAV } from "./lib/nav";
import { href, useRoute } from "./lib/router";
import { AppProvider, useApp, type Dataset } from "./state";
import { About } from "./views/About";
import { Architecture } from "./views/Architecture";
import { Compare } from "./views/Compare";
import { Dependencies } from "./views/Dependencies";
import { Files } from "./views/Files";
import { Findings } from "./views/Findings";
import { History } from "./views/History";
import { Home } from "./views/Home";
import { Hotspots } from "./views/Hotspots";
import { Licenses, PrivacyPolicy, TermsOfUse } from "./views/Legal";
import { Overview } from "./views/Overview";
import { Project } from "./views/Project";
import { Quality } from "./views/Quality";
import { Reports } from "./views/Reports";
import { Security } from "./views/Security";
import { Settings } from "./views/Settings";
import { TimeMachine } from "./views/TimeMachine";

const VIEWS: Record<string, () => ReactNode> = {
  "/": Home,
  "/overview": Overview,
  "/architecture": Architecture,
  "/dependencies": Dependencies,
  "/history": History,
  "/time-machine": TimeMachine,
  "/files": Files,
  "/hotspots": Hotspots,
  "/quality": Quality,
  "/project": Project,
  "/security": Security,
  "/findings": Findings,
  "/reports": Reports,
  "/compare": Compare,
  "/settings": Settings,
  "/about": About,
  "/privacy": PrivacyPolicy,
  "/terms": TermsOfUse,
  "/licenses": Licenses,
};

function NotFound() {
  return (
    <>
      <h1>Page not found</h1>
      <p>
        There is no view at this address. <a href={href("/")}>Go to the start page</a>.
      </p>
    </>
  );
}

function NoAnalysis() {
  return (
    <>
      <h1>No analysis is open</h1>
      <p>
        This view shows an analysis. <a href={href("/")}>Open or analyze a repository</a> first, or
        try the demo.
      </p>
    </>
  );
}

function Routes() {
  const { ready, dataset } = useApp();
  const route = useRoute();
  const nav = NAV.find((item) => item.path === route.path);
  const View = VIEWS[route.path];

  useEffect(() => {
    const parts = [nav?.label, dataset?.dna.identity.name, "RepoDNA"].filter(Boolean);
    document.title = route.path === "/" ? "RepoDNA" : parts.join(" · ");
  }, [nav, dataset, route.path]);

  useEffect(() => {
    window.scrollTo?.({ top: 0 });
    document.getElementById("main")?.focus({ preventScroll: true });
  }, [route.path]);

  let content: ReactNode;
  if (!ready) {
    content = <p className="muted">Starting…</p>;
  } else if (!View) {
    content = <NotFound />;
  } else if (nav?.needsData && !dataset) {
    content = <NoAnalysis />;
  } else {
    content = <View />;
  }
  return (
    <Shell>
      <ErrorBoundary resetKey={`${route.path}:${dataset?.dna.analysisMetadata.id ?? ""}`}>
        {content}
      </ErrorBoundary>
    </Shell>
  );
}

export function App({ initial = null }: { initial?: Dataset | null }) {
  return (
    <AppProvider initial={initial}>
      <Routes />
    </AppProvider>
  );
}
