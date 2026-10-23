import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import AskPopup from "./ask/AskPopup";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <AskPopup />
  </StrictMode>,
);
