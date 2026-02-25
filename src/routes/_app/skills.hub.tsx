import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { SkillsHub } from "../../skills";
import { useAuth } from "../../shared/context";

export const Route = createFileRoute("/_app/skills/hub")({
  component: SkillsHubRoute,
});

function SkillsHubRoute() {
  const navigate = useNavigate();
  const { accountId } = useAuth();

  return (
    <SkillsHub
      accountId={accountId!}
      onBack={() => navigate({ to: "/lines" })}
      onNewSkill={() => navigate({ to: "/skills/studio", search: { skillId: undefined, prompt: undefined } })}
      onEditSkill={(skillId) => navigate({ to: "/skills/studio", search: { skillId, prompt: undefined } })}
    />
  );
}
