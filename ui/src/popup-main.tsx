import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import Popup from "./Popup";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./styles.css";
import { initTheme } from "./theme";

initTheme();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <Popup />
    </ErrorBoundary>
  </StrictMode>,
);
