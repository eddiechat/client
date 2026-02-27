import { createContext, useContext } from "react";

export type ChatFilter = "all" | "1:1" | "3+";

export const ChatFilterContext = createContext<ChatFilter>("all");

export function useChatFilter(): ChatFilter {
  return useContext(ChatFilterContext);
}
