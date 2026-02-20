import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { OnboardingScreen } from "../shared/components";
import { useAuth } from "../shared/context";
import { useEffect } from "react";

export const Route = createFileRoute("/onboarding")({
  component: OnboardingRoute,
});

function OnboardingRoute() {
  const navigate = useNavigate();
  const { accountId, loggedIn } = useAuth();

  useEffect(() => {
    if (!loggedIn) navigate({ to: "/login" });
  }, [loggedIn, navigate]);

  if (!accountId) return null;

  return (
    <OnboardingScreen
      accountId={accountId}
      onComplete={() => navigate({ to: "/points" })}
    />
  );
}
