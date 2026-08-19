import React from "react";
import ReactDOM from "react-dom/client";

import { App } from "./windows/main/App";
import "./styles/global.css";
import { lockDownWebview } from "./lib/webview";

lockDownWebview();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
