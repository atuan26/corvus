import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import SnipOverlay from "./SnipOverlay";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./styles.css";
import { initTheme } from "./theme";

initTheme();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <SnipOverlay />
    </ErrorBoundary>
  </StrictMode>,
);
