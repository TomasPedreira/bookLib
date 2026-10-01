export function validateBackup(data) {
  if (!data || ![1, 2].includes(data.version)
      || !Array.isArray(data.books) || !Array.isArray(data.readings) || !Array.isArray(data.progress_entries)) {
    throw new Error('Invalid backup file');
  }
  return data;
}
