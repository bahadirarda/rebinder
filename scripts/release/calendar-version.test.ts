import { describe, expect, test } from "bun:test";
import {
  isCalendarVersion,
  nextCalendarVersion,
  parseCalendarVersion,
} from "./calendar-version.ts";

describe("calendar version", () => {
  test("parses a canonical release", () => {
    expect(parseCalendarVersion("0.20260817.3")).toEqual({
      epoch: 0,
      date: "20260817",
      revision: 3,
    });
  });

  test("increments another release on the same day", () => {
    expect(nextCalendarVersion("0.20260817.3", "2026-08-17")).toBe("0.20260817.4");
  });

  test("resets the revision on a new day", () => {
    expect(nextCalendarVersion("0.20260817.3", "2026-08-18")).toBe("0.20260818.0");
  });

  test("rejects invalid days and backward dates", () => {
    expect(isCalendarVersion("0.20260230.0")).toBe(false);
    expect(() => nextCalendarVersion("0.20260817.0", "2026-08-16")).toThrow();
  });
});
