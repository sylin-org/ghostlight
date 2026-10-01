// Execute the shipped recipes against a native fake CLI, including a Windows GUI-subsystem
// child. Its receipts and invocation log prove control flow and exit capture independently.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scratchRoot = join(repository, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "journey-recipes-"));
const fake = join(scratch, process.platform === "win32" ? "fake ghostlight.exe" : "fake ghostlight");
const source = join(scratch, "fake.rs");
const fixture = String.raw`
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use std::{env, fs::{self, OpenOptions}, io::Write, process};
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    assert_eq!(args[0], "call");
    assert_eq!(args[3], "--json");
    let tool = &args[1];
    let body = &args[2];
    let step = match tool.as_str() {
        "browser_navigate" => "open",
        "browser_read" => "read",
        "browser_screenshot" => "screenshot",
        "browser_tabs" if body.contains("close") => "close",
        "browser_tabs" => "list",
        _ => panic!("unexpected tool"),
    };
    let mut trace = OpenOptions::new().create(true).append(true).open(env::var("RECIPE_TRACE").unwrap()).unwrap();
    writeln!(trace, "{}\t{}", step, body).unwrap();
    // More than a pipe's capacity: recipes must drain stderr as well as stdout.
    eprintln!("fixture diagnostic {}", "d".repeat(131072));
    let scenario = env::var("RECIPE_SCENARIO").unwrap();
    if scenario == "no_result_6" { process::exit(6); }
    if scenario == "bad_result_0" { println!("not json"); return; }
    let (status, code) = if scenario == format!("{}_6", step) { ("unknown", 6) }
        else if scenario == format!("{}_4", step) { ("failed", 4) }
        else if step == "close" && scenario != "success" { ("blocked", 2) }
        else { ("succeeded", 0) };
    if step == "screenshot" && code == 0 {
        let index = args.iter().position(|arg| arg == "--output").unwrap();
        fs::write(&args[index + 1], [0xff, 0xd8, 0xff, 0xd9]).unwrap();
    }
    println!("{{\"status\":\"{}\",\"summary\":\"{} fixture result\",\"facts\":{{\"tab\":\"tab_recipe\"}}}}", status, step);
    process::exit(code);
}
`;

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repository, encoding: "utf8", windowsHide: true, timeout: 60000,
    maxBuffer: 4 * 1024 * 1024, ...options
  });
  if (result.error) throw result.error;
  return result;
}

function available(command) {
  return spawnSync(command, ["--version"], { windowsHide: true, encoding: "utf8" }).status === 0;
}

try {
  writeFileSync(source, fixture);
  const compiled = run(process.env.RUSTC || "rustc", [source, "--edition=2021", "-o", fake]);
  assert.equal(compiled.status, 0, `Compile native fixture: ${compiled.stderr}`);
  if (process.platform === "win32") {
    const image = readFileSync(fake);
    const optionalHeader = image.readUInt32LE(0x3c) + 24;
    assert.equal(image.readUInt16LE(optionalHeader + 68), 2, "fixture must use the GUI subsystem");
  }

  const shells = [];
  const pwsh = process.env.GHOSTLIGHT_RECIPE_PWSH || (process.platform === "win32" ? "pwsh.exe" : "pwsh");
  if (available(pwsh)) {
    shells.push({ name: "PowerShell", command: pwsh, args: (output) => [
      "-NoProfile", "-File", join(repository, "scripts/browser-journey.ps1"),
      "-Ghostlight", fake, "-Url", 'https://example.com/a?text="hello world"', "-OutputPath", output
    ] });
  } else {
    console.log(`SKIP PowerShell recipe: ${pwsh} is unavailable`);
  }
  const sh = process.env.GHOSTLIGHT_RECIPE_SH || "sh";
  const shellProbe = spawnSync(sh, ["-c", "command -v jq"], { windowsHide: true, encoding: "utf8" });
  if (shellProbe.status === 0) {
    // Git's MSYS shell accepts forward-slash Windows paths and launches the native fixture.
    const portable = (path) => path.replaceAll("\\", "/");
    shells.push({ name: "POSIX", command: sh, args: (output) => [
      portable(join(repository, "scripts/browser-journey.sh")), "--ghostlight", portable(fake),
      "--url", 'https://example.com/a?text="hello world"', "--output", portable(output)
    ] });
  } else {
    console.log(`SKIP POSIX recipe: ${sh} or its required jq is unavailable`);
  }
  assert.ok(shells.length, "At least one recipe shell is required");
  const steps = ["open", "list", "read", "screenshot", "close"];
  const cases = [
    ["success", 0, steps],
    ["open_6", 6, steps.slice(0, 1)],
    ["list_6", 6, steps.slice(0, 2)],
    ["read_6", 6, steps.slice(0, 3)],
    ["screenshot_6", 6, steps.slice(0, 4)],
    ["read_4", 4, steps.slice(0, 3)],
    ["close_2", 2, steps],
    ["no_result_6", 6, steps.slice(0, 1)],
    ["bad_result_0", 1, steps.slice(0, 1)]
  ];
  for (const shell of shells) {
    for (const [scenario, exit, expectedSteps] of cases) {
      const trace = join(scratch, `${shell.name}-${scenario}.trace`);
      const output = join(scratch, `${shell.name}-${scenario} capture.jpg`);
      const result = run(shell.command, shell.args(output), {
        env: { ...process.env, RECIPE_TRACE: trace, RECIPE_SCENARIO: scenario }
      });
      assert.equal(result.status, exit, `${shell.name} ${scenario}: ${result.stdout}\n${result.stderr.slice(-2000)}`);
      const calls = readFileSync(trace, "utf8").trim().split("\n").map((line) => {
        const [step, body] = line.split("\t");
        return { step, body: JSON.parse(body) };
      });
      assert.deepEqual(calls.map(({ step }) => step), expectedSteps, `${shell.name} ${scenario}: dependent calls or retry`);
      assert.equal(calls[0].body.reuse, "never", "journey must open a fresh tab");
      assert.equal(calls[0].body.new_tab, true, "repeat runs must not navigate an earlier controlled tab");
      assert.equal(calls[0].body.url, 'https://example.com/a?text="hello world"', "JSON argument quoting survives");
      for (const { step, body } of calls) {
        if (["read", "screenshot", "close"].includes(step)) assert.equal(body.tab, "tab_recipe");
      }
      assert.ok(result.stderr.includes("fixture diagnostic"), "child diagnostics are retained");
      if (scenario === "success") {
        assert.ok(result.stdout.includes("Journey complete."), "success is reported");
        assert.deepEqual(readFileSync(output), Buffer.from([0xff, 0xd8, 0xff, 0xd9]));
      } else {
        assert.equal(result.stdout.includes("Journey complete."), false, "unsuccessful work must not claim completion");
      }
      if (["open_6", "list_6", "read_6", "read_4", "no_result_6", "bad_result_0"].includes(scenario)) {
        assert.equal(existsSync(output), false, "no capture after an unsuccessful prerequisite");
      }
    }
    console.log(`PASS ${shell.name}: ${cases.length} native-process recipe cases, first outcome preserved and no replay`);
  }
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
