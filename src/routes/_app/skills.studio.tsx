import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { SkillStudio } from "../../skills";
import { useAuth } from "../../shared/context";

export const Route = createFileRoute("/_app/skills/studio")({
  validateSearch: (search: Record<string, unknown>) => ({
    skillId: (search.skillId as string) || undefined,
    prompt: (search.prompt as string) || undefined,
  }),
  component: SkillStudioRoute,
});

function SkillStudioRoute() {
  const navigate = useNavigate();
  const { accountId } = useAuth();
  const { skillId, prompt } = Route.useSearch();

  return (
    <SkillStudio
      accountId={accountId!}
      skillId={skillId}
      initialPrompt={prompt}
      onBack={() => navigate({ to: "/skills/hub" })}
      onSaved={() => navigate({ to: "/skills/hub" })}
      onDeleted={() => navigate({ to: "/skills/hub" })}
    />
  );
}
