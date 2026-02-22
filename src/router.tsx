import { createRouter, createHashHistory } from "@tanstack/react-router";
import { routeTree } from "./routeTree.gen";
import { ErrorFallback } from "./shared/components";

export interface RouterContext {
  auth: {
    loggedIn: boolean;
  };
}

const hashHistory = createHashHistory();

export const router = createRouter({
  routeTree,
  history: hashHistory,
  context: { auth: { loggedIn: false } },
  defaultErrorComponent: ErrorFallback,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
