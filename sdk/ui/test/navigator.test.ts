// The page's navigator says the system raea's window runs on (globalThis.__raea_window.os, set by the window before the
// page's bundle), so a library choosing ⌘ or Ctrl by navigator.platform picks Ctrl on Windows and Linux. Until 0.1.8
// a Windows PC's page said MacIntel. With no window (a render) it's a Mac's, as it always was.
import { afterEach, beforeAll, expect, test } from "bun:test";
import { install } from "../src/dom";
import { navigatorSystem } from "../src/dom/core";

const g = globalThis as any;
beforeAll(() => install());
afterEach(() => delete g.__raea_window);

test("on Windows the navigator says Windows, read when asked (the window's word comes after the DOM is installed)", () => {
  g.__raea_window = { chrome: "native", os: "windows" };
  expect(g.navigator.platform).toBe("Win32");
  expect(g.navigator.userAgent).toContain("Windows NT 10.0; Win64; x64");
  expect(g.navigator.userAgent).not.toContain("Mac");
  expect(g.navigator.userAgentData.platform).toBe("Windows");
});

test("on Linux it says Linux", () => {
  g.__raea_window = { chrome: "native", os: "linux" };
  expect(g.navigator.platform).toBe("Linux x86_64");
  expect(g.navigator.userAgent).toContain("X11; Linux x86_64");
  expect(g.navigator.userAgentData.platform).toBe("Linux");
});

test("on a Mac, and with no window at all, it's a Mac's as before", () => {
  g.__raea_window = { chrome: "inset", os: "macos" };
  expect(g.navigator.platform).toBe("MacIntel");
  delete g.__raea_window;
  expect(g.navigator.platform).toBe("MacIntel");
  expect(g.navigator.userAgent).toContain("Macintosh");
  expect(navigatorSystem().data).toBe("macOS");
});

test("a library's Mac test, as CodeMirror and ProseMirror write it, is false on Windows", () => {
  g.__raea_window = { chrome: "native", os: "windows" };
  expect(/Mac/.test(g.navigator.platform)).toBe(false);
  expect(/Mac|iP(hone|[oa]d)/.test(g.navigator.platform)).toBe(false);
});
