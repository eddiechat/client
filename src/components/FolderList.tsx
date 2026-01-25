import type { Folder } from "../types";

interface FolderListProps {
  folders: Folder[];
  currentFolder: string;
  onFolderSelect: (folder: string) => void;
  loading?: boolean;
}

export function FolderList({
  folders,
  currentFolder,
  onFolderSelect,
  loading,
}: FolderListProps) {
  if (loading) {
    return <div className="folder-list loading">Loading folders...</div>;
  }

  // Common folder icons
  const getFolderIcon = (name: string) => {
    const lower = name.toLowerCase();
    if (lower === "inbox") return "📥";
    if (lower.includes("sent")) return "📤";
    if (lower.includes("draft")) return "📝";
    if (lower.includes("trash") || lower.includes("deleted")) return "🗑️";
    if (lower.includes("spam") || lower.includes("junk")) return "⚠️";
    if (lower.includes("archive")) return "📦";
    return "📁";
  };

  return (
    <nav className="folder-list">
      <h3>Folders</h3>
      <ul>
        {folders.map((folder) => (
          <li
            key={folder.name}
            className={currentFolder === folder.name ? "active" : ""}
            onClick={() => onFolderSelect(folder.name)}
          >
            <span className="folder-icon">{getFolderIcon(folder.name)}</span>
            <span className="folder-name">{folder.name}</span>
          </li>
        ))}
      </ul>
    </nav>
  );
}
