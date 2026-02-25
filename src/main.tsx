import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";
import { router } from "./router";
import { AuthProvider, useAuth } from "./shared/context";
import { DataProvider } from "./shared/context";
import { ThemeProvider } from "./shared/context";

function InnerApp() {
  const auth = useAuth();
  return <RouterProvider router={router} context={{ auth: { loggedIn: auth.loggedIn } }} />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <AuthProvider>
        <DataProvider>
          <InnerApp />
        </DataProvider>
      </AuthProvider>
    </ThemeProvider>
  </React.StrictMode>,
);
