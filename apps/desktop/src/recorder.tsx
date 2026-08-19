import React from "react";
import ReactDOM from "react-dom/client";

import { Recorder } from "./windows/recorder/Recorder";
import "./styles/global.css";
import { lockDownWebview } from "./lib/webview";

lockDownWebview();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Recorder />
  </React.StrictMode>,
);
