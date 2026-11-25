// The page the realtime bench drives (bench/realtime/stage.sh): nothing but the engine's stage, its image filling the
// window at one window point a stage pixel, so a capture of the window is the stage's own frame. One action,
// `rt.call {name, args}`, makes any stage host call (engine/page/stage: stage.show, stage.seek, stage.props, ...) and
// answers what it answered, so a driver outside the page plays the stage as the app's useStage does; `rt.calls
// {calls: [{name, args}]}` makes several at once without waiting between them, as a drag sends props (each answer, in
// order, and the milliseconds to each).
import "@raea/ui/dom-globals";
import { mount } from "@raea/ui/react";
import { call, emit, on } from "@raea/ui/wire";

emit("actions", [
  { name: "rt.call", about: "a stage host call: {name, args}", args: {} },
  { name: "rt.calls", about: "several stage host calls sent at once: {calls: [{name, args}]}", args: {} },
]);
type One = { name: string; args?: unknown };
on<{ id: number; name: string; args?: One & { calls?: One[] } }>("action", ({ id, name, args }) => {
  if (name === "rt.calls" && Array.isArray(args?.calls)) {
    const t0 = performance.now();
    const sent = args.calls.map((c) => call(c.name, c.args ?? null).then((value) => ({ value: value === undefined ? null : JSON.parse(JSON.stringify(value)), ms: performance.now() - t0 }), (e: unknown) => ({ error: String((e as Error)?.message ?? e), ms: performance.now() - t0 })));
    return void Promise.all(sent).then((value) => emit("actionDone", { id, value }));
  }
  if (name !== "rt.call" || !args?.name) return emit("actionDone", { id, error: `no action ${name}` });
  call(args.name, args.args ?? null).then(
    (value) => emit("actionDone", { id, value: value === undefined ? null : JSON.parse(JSON.stringify(value)) }),
    (e: unknown) => emit("actionDone", { id, error: String((e as Error)?.message ?? e) }),
  );
});

mount(
  <div style={{ position: "absolute", left: 0, top: 0, width: "100%", height: "100%", margin: 0, background: "#000" }}>
    <img data-raea-stage="" data-testid="stage" alt="" style={{ position: "absolute", left: 0, top: 0, width: "100%", height: "100%" }} />
  </div>,
  undefined,
  { dom: true },
);
