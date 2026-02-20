import { createRouter, createHashHistory } from "@tanstack/react-router";
import { routeTree } from "./routeTree.gen";

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
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
