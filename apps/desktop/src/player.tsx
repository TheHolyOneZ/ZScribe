import React from "react";
import ReactDOM from "react-dom/client";

import { PlayerWindow } from "./windows/player/PlayerWindow";
import "./styles/global.css";
import { lockDownWebview } from "./lib/webview";

lockDownWebview();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <PlayerWindow />
  </React.StrictMode>,
);
