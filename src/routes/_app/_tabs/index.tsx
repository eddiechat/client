import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/_tabs/")({
  beforeLoad: () => {
    throw redirect({ to: "/points" });
  },
});
