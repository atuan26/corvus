import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import AskPopup from "./ask/AskPopup";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./styles.css";
import { initTheme } from "./theme";

initTheme();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <AskPopup />
    </ErrorBoundary>
  </StrictMode>,
);
