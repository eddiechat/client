import { createContext, useContext } from "react";

export const SearchContext = createContext<string>("");

export function useTabSearch(): string {
  return useContext(SearchContext);
}
