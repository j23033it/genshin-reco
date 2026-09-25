import React from "react";
import ReactDOM from "react-dom/client";
import ResearchChatPrototype from "./features/research-chat/ResearchChatPrototype";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ResearchChatPrototype />
  </React.StrictMode>,
);
