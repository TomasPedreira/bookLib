export function readingDays(progress) {
  const days = new Map();
  for (const entry of progress) {
    const day = entry.recorded_at.slice(0, 10);
    const existing = days.get(day);
    if (!existing || entry.id > existing.id) days.set(day, {...entry, day});
  }
  let previous = 0;
  return [...days.values()].sort((a, b) => a.day.localeCompare(b.day)).map(entry => {
    const result = {...entry, previous, pages: Math.max(0, entry.value - previous)};
    previous = entry.value;
    return result;
  }).reverse();
}
