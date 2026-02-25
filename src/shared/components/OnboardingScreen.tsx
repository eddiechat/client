import { useState, useEffect, useRef, useCallback } from "react";
import {
  getOnboardingStatus,
  onSyncStatus,
  onConversationsUpdated,
  onOnboardingComplete,
} from "../../tauri";
import type { TrustContact } from "../../tauri";
import { useData } from "../context";
import { Avatar } from "./Avatar";

interface OnboardingScreenProps {
  accountId: string;
  onComplete: () => void;
}

type StepDef = {
  label: string;
  done: boolean;
  active: boolean;
  visible: boolean;
};

export function OnboardingScreen({ accountId, onComplete }: OnboardingScreenProps) {
  const data = useData();
  const [statusMessage, setStatusMessage] = useState("");
  const [messageCount, setMessageCount] = useState(0);
  const [trustContacts, setTrustContacts] = useState<TrustContact[]>([]);
  const [trustContactCount, setTrustContactCount] = useState(0);
  const [taskMap, setTaskMap] = useState<Record<string, string>>({});
  const [done, setDone] = useState(false);
  const completing = useRef(false);

  const triggerComplete = useCallback(() => {
    if (completing.current) return;
    completing.current = true;
    setDone(true);
    setTimeout(() => {
      data.refresh(accountId).then(() => onComplete());
    }, 1500);
  }, [accountId, data, onComplete]);

  // Fetch status and update local state
  const refreshStatus = useCallback(async () => {
    try {
      const status = await getOnboardingStatus(accountId);
      setMessageCount(status.message_count);
      setTrustContacts(status.trust_contacts);
      setTrustContactCount(status.trust_contact_count);
      const map: Record<string, string> = {};
      for (const t of status.tasks) {
        map[t.name] = t.status;
      }
      setTaskMap(map);
      if (status.is_complete) {
        triggerComplete();
      }
    } catch {
      // Status not available yet (tasks not seeded), retry on next event
    }
  }, [accountId, triggerComplete]);

  useEffect(() => {
    // Seed initial state
    refreshStatus();

    // Subscribe to events
    const unsubs: Promise<() => void>[] = [];

    unsubs.push(
      onSyncStatus((s) => {
        setStatusMessage(s.message);
      })
    );

    unsubs.push(
      onConversationsUpdated(() => {
        refreshStatus();
      })
    );

    unsubs.push(
      onOnboardingComplete(() => {
        triggerComplete();
      })
    );

    return () => {
      unsubs.forEach((p) => p.then((f) => f()));
    };
  }, [refreshStatus, triggerComplete]);

  const tasksSeeded = Object.keys(taskMap).length > 0;
  const trustDone = taskMap["trust_network"] === "done";
  const historyDone = taskMap["historical_fetch"] === "done";
  const connectionDone = taskMap["connection_history"] === "done";

  // Milestones: 0=nothing, 1=seeded, 2=trust, 3=history, 4=connections, 5=done
  const targetMilestone = done ? 5 : connectionDone ? 4 : historyDone ? 3 : trustDone ? 2 : tasksSeeded ? 1 : 0;
  const [displayMilestone, setDisplayMilestone] = useState(0);

  // Advance displayMilestone one step at a time, with minimum 800ms dwell per step.
  // This ensures every step is visibly active even when the backend jumps ahead.
  useEffect(() => {
    if (targetMilestone > displayMilestone) {
      const delay = displayMilestone === 0 ? 0 : 800;
      const timer = setTimeout(() => {
        setDisplayMilestone((prev) => prev + 1);
      }, delay);
      return () => clearTimeout(timer);
    }
  }, [targetMilestone, displayMilestone]);

  // Derive step states from displayMilestone (never jumps, only advances one at a time)
  const dm = displayMilestone;
  const showLower = dm >= 2; // steps 3-6 visible after trust milestone

  const steps: StepDef[] = [
    {
      label: "Connecting to mail server",
      done: dm >= 1,
      active: dm < 1,
      visible: true,
    },
    {
      label: "Building trust network",
      done: dm >= 2,
      active: dm >= 1 && dm < 2,
      visible: true,
    },
    {
      label: "Syncing message history",
      done: dm >= 3,
      active: dm >= 2 && dm < 3,
      visible: showLower,
    },
    {
      label: "Identifying Points & Circles",
      done: dm >= 3,
      active: dm >= 2 && dm < 3,
      visible: showLower,
    },
    {
      label: "Classifying Lines",
      done: dm >= 4,
      active: dm >= 3 && dm < 4,
      visible: showLower,
    },
    {
      label: "Finishing up",
      done: dm >= 5,
      active: dm >= 4 && dm < 5,
      visible: showLower,
    },
  ];

  // Cycle visible contacts every 3s when there are more than 5
  const [contactOffset, setContactOffset] = useState(0);
  useEffect(() => {
    if (trustContacts.length <= 5 || done) return;
    const timer = setInterval(() => {
      setContactOffset((prev) => (prev + 1) % trustContacts.length);
    }, 3000);
    return () => clearInterval(timer);
  }, [trustContacts.length, done]);

  const visibleContacts = trustContacts.length <= 5
    ? trustContacts
    : Array.from({ length: 5 }, (_, i) =>
      trustContacts[(contactOffset + i) % trustContacts.length]
    );

  const PROGRESS_BY_MILESTONE = [3, 8, 30, 65, 92, 100];
  const progress = PROGRESS_BY_MILESTONE[dm] ?? 3;

  return (
    <div className="h-full flex flex-col p-7 bg-bg-primary">
      <div
        className="text-[28px] font-extrabold text-text-primary"
        style={{ letterSpacing: "-0.02em" }}
      >
        {done ? "You're all set!" : "Setting things up..."}
      </div>
      <div className="text-[14px] text-text-muted mt-0.5">
        Everything runs on your device
      </div>

      {/* Progress bar */}
      <div className="h-[5px] rounded-sm bg-bg-tertiary mt-5 mb-6 overflow-hidden">
        <div
          className="h-full rounded-sm transition-[width] duration-300 ease-out"
          style={{
            width: `${progress}%`,
            background: `linear-gradient(90deg, var(--color-accent-green), var(--color-accent-skill))`,
          }}
        />
      </div>

      {/* Steps */}
      <div className="flex flex-col gap-2.5 mb-5">
        {steps.map(
          (step, i) =>
            step.visible && (
              <div key={i} className="flex items-center gap-3">
                <div
                  className={`w-7 h-7 rounded-lg flex items-center justify-center text-sm transition-all duration-400 ${step.done
                    ? "bg-accent-green text-white border-none"
                    : step.active
                      ? "bg-green-bg border-[1.5px] border-green-border text-text-dim"
                      : "bg-bg-tertiary border-[1.5px] border-divider text-text-dim"
                    }`}
                >
                  {step.done ? "\u2713" : step.active ? "\u25CF" : ""}
                </div>
                <span
                  className={`text-[15px] ${step.done
                    ? "text-text-primary font-semibold"
                    : step.active
                      ? "text-accent-green font-bold"
                      : "text-text-dim font-normal"
                    }`}
                >
                  {step.label}
                </span>
              </div>
            )
        )}
      </div>

      {/* Status message */}
      {statusMessage && !done && (
        <div className="text-[12px] text-text-muted mb-4 truncate">
          {statusMessage}
        </div>
      )}

      {/* Trust contacts */}
      {trustDone && trustContacts.length > 0 && (
        <div className="p-3.5 rounded-2xl bg-bg-secondary border border-divider mb-3.5">
          <div className="text-[12px] font-extrabold text-text-dim tracking-[0.06em] mb-2.5">
            TRUST NETWORK &middot; {trustContactCount} trusted contacts
          </div>
          <div className="flex flex-col gap-2">
            {visibleContacts.map((contact, i) => {
              const maxCount = trustContacts[0].message_count;
              const strength =
                contact.message_count > maxCount * 0.5 ? "high" : "medium";
              return (
                <div key={i} className="flex items-center gap-2.5">
                  <Avatar name={contact.name} email={contact.email} size={8} fontSize="text-[11px]" className="shrink-0" />
                  <span className="text-[13px] text-text-secondary flex-1">
                    {contact.name}
                  </span>
                  <div className="flex items-center gap-1">
                    <div className="w-8 h-1 rounded-sm bg-bg-tertiary overflow-hidden">
                      <div
                        className="h-full rounded-sm bg-accent-green"
                        style={{
                          width: strength === "high" ? "100%" : "55%",
                        }}
                      />
                    </div>
                    <span className="text-[11px] text-text-muted">
                      {strength}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* On-device AI info */}
      <div className="p-3.5 rounded-2xl bg-green-bg border border-green-border">
        <div className="text-[14px] font-extrabold text-accent-green">
          On-device AI
        </div>
        <div className="text-[13px] text-text-secondary mt-1 leading-relaxed">
          {messageCount === 0
            ? "Analyzing your email history locally..."
            : done
              ? `All done. ${messageCount.toLocaleString()} messages processed entirely on your device.`
              : `${messageCount.toLocaleString()} messages processed on your device so far...`}
        </div>
      </div>

      {/* Enter button */}
      {done && (
        <button
          onClick={onComplete}
          className="mt-4 py-3.5 rounded-2xl bg-accent-green text-white text-center text-lg font-extrabold cursor-pointer w-full border-none"
        >
          Enter Eddie &rarr;
        </button>
      )}
    </div>
  );
}
