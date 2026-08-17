const calendarVersionPattern = /^0\.(\d{8})\.(0|[1-9]\d*)$/;

export interface CalendarVersion {
  epoch: 0;
  date: string;
  revision: number;
}

function calendarStamp(releaseDate: string): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(releaseDate)) {
    throw new Error(`Release date must use YYYY-MM-DD: ${releaseDate}`);
  }
  const parsed = new Date(`${releaseDate}T00:00:00.000Z`);
  if (Number.isNaN(parsed.valueOf()) || parsed.toISOString().slice(0, 10) !== releaseDate) {
    throw new Error(`Release date is not a calendar day: ${releaseDate}`);
  }
  return releaseDate.replaceAll("-", "");
}

export function parseCalendarVersion(version: string): CalendarVersion | null {
  const match = version.match(calendarVersionPattern);
  const date = match?.[1];
  const revision = match?.[2];
  if (!date || revision === undefined) return null;
  calendarStamp(`${date.slice(0, 4)}-${date.slice(4, 6)}-${date.slice(6, 8)}`);
  const parsedRevision = Number(revision);
  if (!Number.isSafeInteger(parsedRevision)) {
    throw new Error(`Calendar revision is outside the safe integer range: ${version}`);
  }
  return { epoch: 0, date, revision: parsedRevision };
}

export function isCalendarVersion(version: string): boolean {
  try {
    return parseCalendarVersion(version) !== null;
  } catch {
    return false;
  }
}

export function nextCalendarVersion(previousVersion: string, releaseDate: string): string {
  const previous = parseCalendarVersion(previousVersion);
  if (!previous) {
    throw new Error(`Previous version must use 0.YYYYMMDD.REVISION: ${previousVersion}`);
  }
  const date = calendarStamp(releaseDate);
  if (date < previous.date) {
    throw new Error(`Release date ${releaseDate} precedes ${previousVersion}`);
  }
  return `0.${date}.${date === previous.date ? previous.revision + 1 : 0}`;
}
