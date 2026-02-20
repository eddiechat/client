import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app")({
  beforeLoad: ({ context }) => {
    if (!context.auth.loggedIn) {
      throw redirect({ to: "/login" });
    }
  },
  component: AppLayout,
});

function AppLayout() {
  return <Outlet />;
}
