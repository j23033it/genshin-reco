import React from "react";
import ReactDOM from "react-dom/client";
import ResearchChatPrototype from "./features/research-chat/ResearchChatPrototype";
import { CodexConnectionGate } from "./features/research-chat/CodexConnectionGate";
import { createDemoResearchRepository } from "./features/research-chat/demoRepository";
import "./styles.css";

const demoRepository =
  import.meta.env.DEV &&
  new URLSearchParams(window.location.search).get("demo") === "1"
    ? createDemoResearchRepository()
    : undefined;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {demoRepository ? (
      <ResearchChatPrototype repository={demoRepository} />
    ) : (
      <CodexConnectionGate />
    )}
  </React.StrictMode>,
);
