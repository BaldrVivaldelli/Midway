export type CommandPaletteItem = {
  id: string;
  title: string;
  subtitle?: string | null;
  keywords?: string[];
  section: string;
};

function normalize(text: string): string {
  return text.toLowerCase().trim();
}

function scoreItem(query: string, item: CommandPaletteItem): number {
  const q = normalize(query);
  if (!q) {
    return 0;
  }

  const title = normalize(item.title);
  const subtitle = normalize(item.subtitle ?? "");
  const keywords = (item.keywords ?? []).map(normalize);

  if (title === q) {
    return 100;
  }
  if (title.startsWith(q)) {
    return 80;
  }
  if (keywords.some((keyword) => keyword === q)) {
    return 72;
  }
  if (title.includes(q)) {
    return 60;
  }
  if (keywords.some((keyword) => keyword.startsWith(q))) {
    return 48;
  }
  if (subtitle.includes(q)) {
    return 36;
  }
  if (keywords.some((keyword) => keyword.includes(q))) {
    return 28;
  }

  const titleTokens = title.split(/\s+/).filter(Boolean);
  if (titleTokens.some((token) => token.startsWith(q))) {
    return 24;
  }

  return -1;
}

export function searchPaletteItems(
  items: CommandPaletteItem[],
  query: string,
  limit = 12
): CommandPaletteItem[] {
  const normalizedQuery = normalize(query);
  if (!normalizedQuery) {
    return items.slice(0, limit);
  }

  return items
    .map((item) => ({ item, score: scoreItem(normalizedQuery, item) }))
    .filter((entry) => entry.score >= 0)
    .sort((left, right) => {
      if (right.score !== left.score) {
        return right.score - left.score;
      }

      return left.item.title.localeCompare(right.item.title);
    })
    .slice(0, limit)
    .map((entry) => entry.item);
}
