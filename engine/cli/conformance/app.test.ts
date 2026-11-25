// `raea show` and its app on a Mac, Windows and Linux (src/app.ts), each system faked (a Here): where the running
// app's host is, which app is installed, how files open without it, how it's started, and the harness found up the
// processes. #169: on Windows `raea show` asked `kill` whether the app ran, looked only in /Applications, and said
// it had opened the files "in the Mac's own viewer".
import { afterAll, describe, expect, test } from "bun:test";
import { appHost, computer, harness, installedApp, openFiles, show, type Here } from "../src/app";
import { pixelsOf } from "../src/brand";
import { absolute, fromCwd } from "../src/show";

const WIN_KEY = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\raea";

/** A system: `platform`, its files (path → text), and its programs' answers; every run is kept in `ran`. */
function here(platform: string, files: Record<string, string> = {}, answer: (cmd: string, args: string[]) => { status?: number; stdout?: string } = () => ({})) {
  const ran: string[][] = [];
  const h: Here & { ran: string[][] } = {
    platform,
    ran,
    run: (cmd, args) => {
      ran.push([cmd, ...args]);
      const a = answer(cmd, args);
      return { status: a.status ?? 0, stdout: a.stdout ?? "" };
    },
    exists: (p) => p in files,
    read: (p) => {
      if (!(p in files)) throw new Error(`ENOENT ${p}`);
      return files[p];
    },
    env: { RAEA_NO_OPEN: undefined },
    home: platform === "win32" ? "C:/Users/me" : "/home/you",
  };
  return h;
}

// the app's host, as the window's (apps/app host/show.ts): an item's link for a show, and /host/shown to say it finds it.
// Its own process: show() posts with a synchronous curl, which a server in this one couldn't answer
const host = Bun.spawn(["bun", "-e", `const s = Bun.serve({ port: 0, fetch: (r) => { const p = new URL(r.url).pathname; return p === "/host/show" ? Response.json({ url: "raea://item/7" }) : p === "/host/shown" ? Response.json({}) : new Response("not found", { status: 404 }); } }); console.log(s.port);`], { stdout: "pipe" });
afterAll(() => host.kill());
/** its first line: the port it listens on */
async function portOf(out: ReadableStream<Uint8Array>) {
  const r = out.getReader();
  let text = "";
  while (!text.includes("\n")) {
    const { value, done } = await r.read();
    if (done) throw new Error(`the test's host stopped before it listened: ${text}`);
    text += new TextDecoder().decode(value);
  }
  r.releaseLock();
  return Number(text.trim());
}
const HOST = `http://127.0.0.1:${await portOf(host.stdout)}`;

describe("the running app's host", () => {
  test("on Windows, its pid is asked of tasklist (Windows has no kill)", () => {
    const file = { "C:/Users/me/.raea/app-host.json": JSON.stringify({ url: HOST, pid: 4321 }) };
    const up = here("win32", file, (cmd, args) => (cmd === "tasklist" && args.includes("PID eq 4321") ? { stdout: '"raea.exe","4321","Console","1","80,000 K"\r\n' } : {}));
    expect(appHost(up)).toBe(HOST);
    expect(up.ran.some(([c]) => c === "kill")).toBe(false);
    // a file a stopped app left: tasklist finds no such process ("INFO: No tasks are running...")
    const gone = here("win32", file, () => ({ stdout: "INFO: No tasks are running which match the specified criteria.\r\n" }));
    expect(appHost(gone)).toBeNull();
  });

  test("on a Mac or Linux, kill -0", () => {
    const file = { "/home/you/.raea/app-host.json": JSON.stringify({ url: HOST, pid: 99 }) };
    expect(appHost(here("linux", file, (cmd) => ({ status: cmd === "kill" ? 0 : 1 })))).toBe(HOST);
    expect(appHost(here("linux", file, (cmd) => ({ status: cmd === "kill" ? 1 : 0 })))).toBeNull();
  });
});

describe("the installed app", () => {
  test("on Windows, raea.exe in the folder its installer was given (Settings > Apps keeps it)", () => {
    const reg = `\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\raea\r\n    InstallLocation    REG_SZ    D:\\Apps\\raea\r\n\r\n`;
    const up = here("win32", { "D:\\Apps\\raea\\raea.exe": "" }, (cmd, args) => (cmd === "reg" && args[1] === WIN_KEY ? { stdout: reg } : {}));
    expect(installedApp(up)).toBe("D:\\Apps\\raea\\raea.exe");
  });

  test("on Windows with no InstallLocation, the default folder", () => {
    const up = here("win32", { "C:\\Users\\me\\AppData\\Local\\Programs\\raea\\raea.exe": "" }, () => ({ status: 1 }));
    up.env.LOCALAPPDATA = "C:\\Users\\me\\AppData\\Local";
    expect(installedApp(up)).toBe("C:\\Users\\me\\AppData\\Local\\Programs\\raea\\raea.exe");
  });

  test("on a Mac, raea.app in Applications; on Linux, the launcher raea.desktop runs", () => {
    expect(installedApp(here("darwin", { "/Applications/raea.app": "" }))).toBe("/Applications/raea.app");
    const desktop = "[Desktop Entry]\nName=raea\nExec=\"/home/you/.local/share/raea-app/launch.sh\"\n";
    expect(installedApp(here("linux", { "/home/you/.local/share/applications/raea.desktop": desktop, "/home/you/.local/share/raea-app/launch.sh": "" }))).toBe("/home/you/.local/share/raea-app/launch.sh");
    expect(installedApp(here("linux"))).toBeUndefined();
  });
});

describe("raea show without the app", () => {
  test("on Windows, each file opens in Windows' own viewer through Explorer, its path in Windows' slashes", () => {
    const up = here("win32", {}, () => ({ status: 1 }));
    const r = show({ files: ["C:/Users/me/cut & keep/a.mp4", "C:/Users/me/b.png"] }, up) as { opened: string[]; viewer: string };
    expect(r.viewer).toBe("Windows' own viewer");
    expect(up.ran.filter(([c]) => c === "explorer.exe")).toEqual([["explorer.exe", "C:\\Users\\me\\cut & keep\\a.mp4"], ["explorer.exe", "C:\\Users\\me\\b.png"]]);
    expect(up.ran.some(([c]) => c === "open")).toBe(false);
  });

  test("a question needs the app, said for the system it runs on", () => {
    expect(() => show({ files: [], question: "Which?" }, here("win32", {}, () => ({ status: 1 })))).toThrow("isn't on this PC");
    expect(() => show({ files: [], question: "Which?" }, here("darwin"))).toThrow("isn't on this Mac");
    expect(computer(here("linux")).name).toBe("this computer");
  });

  test("on Linux, xdg-open", () => {
    const up = here("linux");
    openFiles(["/home/you/a.mp4"], up);
    expect(up.ran).toEqual([["xdg-open", "/home/you/a.mp4"]]);
  });
});

describe("raea show starting the app", () => {
  test("on Windows: raea.exe started through Explorer, its host waited for, the item posted, no raea:// link", () => {
    const files: Record<string, string> = { "C:\\Users\\me\\AppData\\Local\\Programs\\raea\\raea.exe": "" };
    const up = here("win32", files, (cmd, args) => {
      // the app, once started, writes its host's address
      if (cmd === "explorer.exe" && args[0].endsWith("raea.exe")) files["C:/Users/me/.raea/app-host.json"] = JSON.stringify({ url: HOST, pid: 77 });
      if (cmd === "tasklist") return { stdout: '"raea.exe","77","Console","1","1 K"' };
      return { status: cmd === "reg" ? 1 : 0 };
    });
    up.env.LOCALAPPDATA = "C:\\Users\\me\\AppData\\Local";
    const r = show({ files: ["C:/Users/me/a.mp4"] }, up) as { url: string; brought: boolean };
    expect(r).toEqual({ url: "raea://item/7", brought: false });
    expect(up.ran.some(([c]) => c === "open" || c === "pgrep")).toBe(false);
  });

  test("on a Mac: open -a, and the window brought up on the item it was started for", () => {
    const files: Record<string, string> = { "/Applications/raea.app": "" };
    const up = here("darwin", files, (cmd, args) => {
      if (cmd === "open" && args[0] === "-a") files["/home/you/.raea/app-host.json"] = JSON.stringify({ url: HOST, pid: 77 });
      return { status: cmd === "pgrep" ? 1 : 0 };
    });
    expect(show({ files: ["/tmp/a.mp4"] }, up)).toEqual({ url: "raea://item/7", brought: true });
    expect(up.ran.at(-1)).toEqual(["open", "raea://item/7"]);
  });
});

describe("the harness that ran raea show", () => {
  test("on Windows, found up every process's record from one PowerShell call, claude.exe as claude", () => {
    const list = [`${process.pid}\t500\t"C:\\Users\\me\\raea\\bin\\raea.exe" show a.mp4`, `500\t400\t"C:\\Users\\me\\.local\\bin\\claude.exe" --resume`, `400\t1\tC:\\Windows\\explorer.exe`].join("\r\n");
    const up = here("win32", {}, (cmd) => (cmd === "powershell" ? { stdout: list } : { status: 1 }));
    expect(harness(up)).toEqual({ name: "Claude Code", pid: 500 });
    expect(up.ran.filter(([c]) => c === "powershell")).toHaveLength(1);
  });

  test("a runtime's script by its name, the quoted path's spaces kept", () => {
    const list = [`${process.pid}\t600\traea.exe show`, `600\t1\t"C:\\Program Files\\nodejs\\node.exe" C:\\Users\\me\\AppData\\Roaming\\npm\\gemini.cmd`].join("\n");
    expect(harness(here("win32", {}, () => ({ stdout: list })))).toEqual({ name: "Gemini", pid: 600 });
  });

  test("on a Mac or Linux, ps up from the parent", () => {
    const up = here("linux", {}, (cmd, args) => (cmd === "ps" && args.at(-1) === String(process.ppid) ? { stdout: " 1 /usr/local/bin/codex exec" } : { stdout: "" }));
    expect(harness(up)).toEqual({ name: "Codex", pid: process.ppid });
  });
});

describe("a path from the cwd", () => {
  test("a Windows one is absolute, with either slash, a share's too", () => {
    for (const p of ["C:\\Users\\me\\a.mp4", "c:/x", "\\\\nas\\clips\\a.mp4", "\\x", "/x"]) expect(absolute(p)).toBe(true);
    for (const p of ["a.mp4", "cuts\\a.mp4", "C:a.mp4", "./a"]) expect(absolute(p)).toBe(false);
    expect(fromCwd("C:/Users/me/proj", "D:\\clips\\a.mp4")).toBe("D:\\clips\\a.mp4");
    expect(fromCwd("C:/Users/me/proj/", "out\\a.png")).toBe("C:/Users/me/proj/out\\a.png");
    expect(fromCwd("/home/you", "a.png")).toBe("/home/you/a.png");
  });
});

describe("a logo's pixels, on any system", () => {
  // until 0.1.8 asked of sips, a Mac's: on Windows and Linux every raster logo in brand.json had no size. Each is the
  // head of a 37x23 image from a real encoder (ffmpeg's PNG, GIF and ICO; cwebp's lossy and lossless; Pillow's WebP with
  // alpha, VP8X; sips's JPEG, an Exif and a Photoshop block before its frame; ImageMagick's progressive JPEG)
  const HEADS: Record<string, string> = {
    png: "iVBORw0KGgoAAAANSUhEUgAAACUAAAAX",
    gif: "R0lGODlhJQAXAA==",
    "webp lossy": "UklGRiACAABXRUJQVlA4IBQCAAAwDwCdASolABcA",
    "webp lossless": "UklGRrIAAABXRUJQVlA4TKUAAAAvJIAF",
    "webp with alpha": "UklGRkwCAABXRUJQVlA4IEACAABQDwCdASolABcA",
    ico: "AAABAAEAJRc=",
    jpeg: "/9j/4AAQSkZJRgABAQAASABIAAD/4QBMRXhpZgAATU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6ABAAMAAAABAAEAAKACAAQAAAABAAAAJaADAAQAAAABAAAAFwAAAAD/7QA4UGhvdG9zaG9wIDMuMAA4QklNBAQAAAAAAAA4QklNBCUAAAAAABDUHYzZjwCyBOmACZjs+EJ+/8AAEQgAFwAlAw==",
    "jpeg progressive": "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAMCAgICAgMCAgIDAwMDBAYEBAQEBAgGBgUGCQgKCgkICQkKDA8MCgsOCwkJDRENDg8QEBEQCgwSExIQEw8QEBD/2wBDAQMDAwQDBAgEBAgQCwkLEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBD/wgARCAAXACUD",
  };
  for (const [kind, head] of Object.entries(HEADS)) {
    test(kind, () => expect(pixelsOf(Buffer.from(head, "base64"))).toEqual({ width: 37, height: 23 }));
  }
  test("anything else, or a header cut short, has none", () => {
    expect(pixelsOf(new TextEncoder().encode("II*\0 a TIFF"))).toBeUndefined();
    expect(pixelsOf(Buffer.from(HEADS.png, "base64").subarray(0, 18))).toBeUndefined();
    expect(pixelsOf(new Uint8Array())).toBeUndefined();
  });
});
