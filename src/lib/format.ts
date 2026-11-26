const dateFmt = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric" });
const timeFmt = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });

export function formatDate(ms: number): string {
  return ms > 0 ? dateFmt.format(ms) : "";
}

export function formatTime(ms: number): string {
  return timeFmt.format(ms);
}

/** "Today" / "Yesterday" / "N days ago" / date, like Mihon's relative dates. */
export function relativeDay(ms: number): string {
  if (ms <= 0) return "";
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(new Date()) - startOfDay(new Date(ms))) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  return dateFmt.format(ms);
}

/** Groups items under relative-day headers, keeping input order. */
export function groupByDay<T>(items: T[], time: (t: T) => number): { day: string; items: T[] }[] {
  const out: { day: string; items: T[] }[] = [];
  for (const it of items) {
    const day = relativeDay(time(it));
    const last = out[out.length - 1];
    if (last && last.day === day) last.items.push(it);
    else out.push({ day, items: [it] });
  }
  return out;
}
