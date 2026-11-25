//! Node's core on JavaScriptCore (js::run_node): a TypeScript script using fs, path, os, crypto, child_process,
//! process and Buffer, an ES module package resolved through package.json's exports, and a CommonJS package (imported,
//! and required from inside another), each result what Node gives.
#![cfg(feature = "modules")]
use std::time::Duration;

#[test]
fn runs_a_tool_as_node_would() {
    let d = std::env::temp_dir().join(format!("raea-node-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let w = |p: &str, s: &str| {
        let f = d.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, s).unwrap();
    };
    w("node_modules/esm-pkg/package.json", r#"{ "name": "esm-pkg", "type": "module", "exports": { ".": { "import": "./lib/index.js", "require": "./nope.cjs" }, "./extra": "./lib/extra.js" } }"#);
    w("node_modules/esm-pkg/lib/index.js", "export const twice = (x) => x * 2;\nexport default 'esm-default';\n");
    w("node_modules/esm-pkg/lib/extra.js", "export const extra = 'extra';\n");
    w("node_modules/cjs-pkg/package.json", r#"{ "name": "cjs-pkg", "main": "main.js" }"#);
    w("node_modules/cjs-pkg/main.js", "const { join } = require('path');\nconst helper = require('./helper');\nmodule.exports = { joined: join('a', 'b'), helper: helper.name, self: typeof __dirname };\n");
    w("node_modules/cjs-pkg/helper.js", "exports.name = 'helper';\n");
    w("data.json", r#"{ "n": 3 }"#);
    w("tool.ts", r#"
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync, appendFileSync, rmSync } from "fs";
import { join, dirname, basename, relative, extname } from "path";
import { tmpdir } from "os";
import { createHash } from "crypto";
import { spawnSync } from "child_process";
import esm, { twice } from "esm-pkg";
import { extra } from "esm-pkg/extra";
import cjs from "cjs-pkg";
import data from "./data.json";
import { createRequire } from "module";
const out: Record<string, unknown> = {};
const dir = join(process.argv[2], "work");
mkdirSync(join(dir, "deep/er"), { recursive: true });
writeFileSync(join(dir, "a.txt"), "héllo");
appendFileSync(join(dir, "a.txt"), "!");
writeFileSync(join(dir, "b.bin"), new Uint8Array([0, 255, 7]));
out.text = readFileSync(join(dir, "a.txt"), "utf8");
out.bytes = Array.from(readFileSync(join(dir, "b.bin")));
out.isBuffer = Buffer.isBuffer(readFileSync(join(dir, "b.bin")));
out.list = readdirSync(dir).sort();
out.types = readdirSync(dir, { withFileTypes: true }).map((e) => e.name + ":" + (e.isDirectory() ? "d" : "f")).sort();
out.size = statSync(join(dir, "a.txt")).size;
out.mtime = statSync(join(dir, "a.txt")).mtimeMs > 0;
rmSync(join(dir, "deep"), { recursive: true, force: true });
out.gone = existsSync(join(dir, "deep"));
let missing = "";
try { readFileSync(join(dir, "nope")); } catch (e) { missing = (e as any).code; }
out.missing = missing;
out.path = [dirname("/a/b/c.ts"), basename("/a/b/c.ts", ".ts"), extname("x.tar.gz"), relative("/a/b", "/a/c/d"), join("a", "..", "b", "./c")];
out.tmp = tmpdir().length > 0;
out.sha = createHash("sha256").update("abc").digest("hex");
out.sha1 = createHash("sha1").update(Buffer.from("abc")).digest("base64");
const echo = spawnSync("/bin/sh", ["-c", "printf out; printf err >&2; exit 3"]);
out.spawn = [echo.status, echo.stdout.toString(), echo.stderr.toString()];
out.b64 = Buffer.from("héllo").toString("base64") + "|" + Buffer.from("aMOpbGxv", "base64").toString();
out.pkgs = [esm, twice(21), extra, cjs.joined, cjs.helper, cjs.self];
out.json = data.n;
out.argv = process.argv.slice(3);
out.env = typeof process.env.HOME;
out.req = createRequire(import.meta.url)("cjs-pkg").helper;
process.stdout.write(JSON.stringify(out));
process.exit(5);
"#);
    let r = raea_native::js::run_node(&d.join("tool.ts"), &[d.display().to_string(), "x".into(), "y".into()], String::new(), Duration::from_secs(20));
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(r.exit, 5, "process.exit(5); stderr: {}", r.stderr);
    let got: serde_json::Value = serde_json::from_str(&r.stdout).unwrap_or_else(|e| panic!("{e}: {} / {}", r.stdout, r.stderr));
    // what Node 22 gives for the same script
    let want = serde_json::json!({
        "text": "héllo!", "bytes": [0, 255, 7], "isBuffer": true, "list": ["a.txt", "b.bin", "deep"], "types": ["a.txt:f", "b.bin:f", "deep:d"],
        "size": 7, "mtime": true, "gone": false, "missing": "ENOENT",
        "path": ["/a/b", "c", ".gz", "../c/d", "b/c"], "tmp": true,
        "sha": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", "sha1": "qZk+NkcGgWq6PiVxeFDCbJzQ2J0=",
        "spawn": [3, "out", "err"], "b64": "aMOpbGxv|héllo", "pkgs": ["esm-default", 42, "extra", "a/b", "helper", "string"],
        "json": 3, "argv": ["x", "y"], "env": "string", "req": "helper"
    });
    assert_eq!(got, want);
}

#[test]
fn a_script_that_sets_exit_code_exits_with_it() {
    let d = std::env::temp_dir().join(format!("raea-node-exitcode-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("set.ts"), "process.stderr.write(\"refused\\n\");\nprocess.exitCode = 3;\n").unwrap();
    std::fs::write(d.join("plain.ts"), "process.stdout.write(\"ok\");\n").unwrap();
    let set = raea_native::js::run_node(&d.join("set.ts"), &[], String::new(), Duration::from_secs(20));
    let plain = raea_native::js::run_node(&d.join("plain.ts"), &[], String::new(), Duration::from_secs(20));
    let _ = std::fs::remove_dir_all(&d);
    // as Node: the status it set, and what it wrote before ending
    assert_eq!((set.exit, set.stderr.as_str()), (3, "refused\n"));
    assert_eq!((plain.exit, plain.stdout.as_str()), (0, "ok"));
}

/// The system as Node names it, and Windows' paths parsed as Windows does (#169: on a Windows PC raea's Node core said
/// it was an arm64 Mac, process.platform was Rust's "windows", resolve(".") named "/C:\Users\...", no folder at all, and
/// there was no process.ppid on any system, so `raea show` never found the harness that ran it). path.win32 parses
/// Windows' paths on any system, as the Windows path does there, writing them with /. Each answer is Node 22's, its \
/// written as /.
#[test]
fn names_the_system_as_node_does_and_parses_windows_paths() {
    let d = std::env::temp_dir().join(format!("raea-node-system-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("system.ts"), r#"
import { arch, platform, type, tmpdir, homedir } from "os";
import path from "path";
import { fileURLToPath, pathToFileURL } from "url";
const w = path.win32;
process.stdout.write(JSON.stringify({
  system: [process.platform === platform(), process.arch === arch(), process.pid > 1, process.ppid > 1 && process.ppid !== process.pid, type()],
  here: [path.isAbsolute(process.cwd()), path.resolve(".") === process.cwd(), path.isAbsolute(tmpdir()), !/[\\/]$/.test(tmpdir()), path.isAbsolute(homedir())],
  resolve: [w.resolve("C:\\a", "b\\..\\c"), w.resolve("C:\\a", "D:\\x", "y"), w.resolve("\\\\nas\\clips\\", "a.mp4"), w.resolve("C:/a/", "..", "..")],
  absolute: ["C:\\x", "c:/x", "\\\\nas\\clips", "\\x", "x", "C:x"].map((p) => w.isAbsolute(p)),
  parts: [w.dirname("C:\\a\\b.txt"), w.dirname("C:\\a"), w.dirname("C:\\"), w.basename("C:\\a\\b.txt", ".txt"), w.extname("C:\\a.b\\c.mp4"), w.basename("C:\\")],
  join: [w.join("C:\\Users\\me", ".raea", "app-host.json"), w.normalize("C:\\a\\.\\b\\..\\c\\"), w.parse("C:\\a\\b.txt").root],
  relative: [w.relative("C:\\a\\b", "C:\\a\\c\\d"), w.relative("C:\\A\\b", "c:\\a\\B\\x"), w.relative("C:\\a", "D:\\b")],
  delimiter: [w.delimiter, path.posix.delimiter, path.posix.resolve("/a", "b"), path.posix.isAbsolute("C:\\x")],
  url: [fileURLToPath(pathToFileURL(path.resolve("a b#1.txt"))) === path.resolve("a b#1.txt"), pathToFileURL("/x y").href.startsWith("file:///")],
}));
"#).unwrap();
    let r = raea_native::js::run_node(&d.join("system.ts"), &[], String::new(), Duration::from_secs(20));
    let _ = std::fs::remove_dir_all(&d);
    let got: serde_json::Value = serde_json::from_str(&r.stdout).unwrap_or_else(|e| panic!("{e}: {} / {}", r.stdout, r.stderr));
    let kind = match std::env::consts::OS { "macos" => "Darwin", "windows" => "Windows_NT", "linux" => "Linux", os => os };
    let want = serde_json::json!({
        "system": [true, true, true, true, kind],
        "here": [true, true, true, true, true],
        "resolve": ["C:/a/c", "D:/x/y", "//nas/clips/a.mp4", "C:/"],
        "absolute": [true, true, true, true, false, false],
        "parts": ["C:/a", "C:/", "C:/", "b", ".mp4", ""],
        "join": ["C:/Users/me/.raea/app-host.json", "C:/a/c/", "C:/"],
        "relative": ["../c/d", "x", "D:/b"],
        "delimiter": [";", ":", "/a/b", false],
        "url": [true, true],
    });
    assert_eq!(got, want);
    // and Node's own words for this system
    let platform = match std::env::consts::OS { "macos" => "darwin", "windows" => "win32", os => os };
    let probe = d.with_extension("platform.ts");
    std::fs::write(&probe, "process.stdout.write(process.platform + ' ' + process.arch);\n").unwrap();
    let r = raea_native::js::run_node(&probe, &[], String::new(), Duration::from_secs(20));
    let _ = std::fs::remove_file(&probe);
    let arch = match std::env::consts::ARCH { "x86_64" => "x64", "aarch64" => "arm64", a => a };
    assert_eq!(r.stdout, format!("{platform} {arch}"));
}
