// A plan's tools for an agent (docs/plan.feature PL5): plan_read, plan_add, plan_update and plan_close, as the raea
// app offers them to every agent it launches. Any host answers them the same way: it says how to read the plan's text
// and how to make edits on it, and these turn a tool's input into plan edits (edit.ts) and its answer into the open
// tasks with their ids. The app's page answers them through its plans, a headless run on the folder's plan.org.
import { planDigest } from "./digest.ts"
import { applyEdits, type PlanEdit, type TaskFields } from "./edit.ts"
import { find, isClosed, readPlan, type Plan, type State, type Task } from "./model.ts"

export const PLAN_TOOLS = ["plan_read", "plan_add", "plan_update", "plan_close"] as const
export type PlanTool = (typeof PLAN_TOOLS)[number]

/** A task as plan_add takes it. Left without agents, it's the calling agent's. */
export interface NewTaskInput extends Omit<TaskFields, "title"> {
  title: string
  state?: string
  under?: string
  before?: string
  after?: string
  checks?: string[]
}

/** One change as plan_update takes it: fields to set, and at most one of state, check, move or remove. */
export interface TaskChange extends TaskFields {
  id: string
  state?: string
  /** a checkbox under it, by its place (0 is the first): ticked, or as `done` says */
  check?: number
  done?: boolean
  /** a checkbox added under it */
  add_check?: string
  move?: { before?: string; after?: string; under?: string }
  remove?: boolean
}

export interface PlanToolHost {
  /** the calling agent's id: a new task with no agents is its */
  who: string
  /** the plan's text now */
  text(): string
  /** the edits made on the plan: its text after, and each edit's task id */
  apply(edits: PlanEdit[]): Promise<{ text: string; refs: string[] }> | { text: string; refs: string[] }
  /** where the plan is, for the agent to say or open */
  file?: string
}

const list = <T>(v: unknown, what: string): T[] => {
  if (!Array.isArray(v) || !v.length) throw new Error(`${what}: a list, with at least one`)
  return v as T[]
}

/** The fields of a change that set a task's own values (title, priority, tags, dates, item, agents, properties, note). */
function fieldsOf(c: TaskChange): TaskFields | undefined {
  const { id: _, state: _s, check: _c, done: _d, add_check: _a, move: _m, remove: _r, ...f } = c
  return Object.keys(f).length ? f : undefined
}

/** A tool's input as plan edits, in order. Throws, naming what's wrong, on an input that isn't one. */
export function planToolEdits(tool: Exclude<PlanTool, "plan_read">, input: Record<string, unknown>, who: string): PlanEdit[] {
  if (tool === "plan_add")
    return list<NewTaskInput>(input.tasks, "tasks").map((t) => {
      if (!t || typeof t.title !== "string" || !t.title.trim()) throw new Error("each task needs a title")
      return { ...t, op: "add" as const, agents: t.agents ?? [who] }
    })
  if (tool === "plan_close") {
    if (typeof input.id !== "string") throw new Error("id: the task to close (plan_read lists them)")
    const state = input.state === undefined ? "done" : String(input.state)
    if (!/^(done|dropped)$/i.test(state) && state !== state.toUpperCase()) throw new Error(`state: done or dropped (or a closing keyword of the plan's), not ${state}`)
    return [{ op: "state", ref: input.id, state, ...(typeof input.item === "string" && { item: input.item }), ...(typeof input.note === "string" && { note: input.note }) }]
  }
  return list<TaskChange>(input.changes, "changes").flatMap((c): PlanEdit[] => {
    if (!c || typeof c.id !== "string") throw new Error("each change names its task by id (plan_read lists them)")
    if (c.remove) return [{ op: "remove", ref: c.id }]
    const out: PlanEdit[] = []
    const f = fieldsOf(c)
    if (f) out.push({ op: "set", ref: c.id, ...f })
    if (c.state) out.push({ op: "state", ref: c.id, state: c.state })
    if (c.check !== undefined) out.push({ op: "check", ref: c.id, index: c.check, ...(c.done !== undefined && { done: c.done }) })
    if (c.add_check) out.push({ op: "check", ref: c.id, add: c.add_check })
    if (c.move) out.push({ op: "move", ref: c.id, ...c.move })
    if (!out.length) throw new Error(`nothing to change on ${c.id}: give a field, state, check, move or remove`)
    return out
  })
}

/** The state a keyword or state name means in this plan. */
const meant = (plan: Plan, state: string): State | undefined => plan.words.get(state) ?? plan.words.get(state.toUpperCase()) ?? (state.toLowerCase() as State)

/** A task and what is still open under it: its unticked checkboxes, by place, and every open task below it. */
function openUnder(plan: Plan, t: Task): { boxes: number[]; tasks: Task[] } {
  const tasks: Task[] = []
  const walk = (ref: string) => {
    for (const r of find(plan, ref)?.children ?? []) {
      const k = find(plan, r)
      if (k?.state && !isClosed(k.state)) tasks.push(k)
      if (k) walk(k.ref)
    }
  }
  walk(t.ref)
  return { boxes: t.checks.flatMap((c, i) => (c.done ? [] : [i])), tasks }
}

const quoted = (s: string) => `"${s.length > 40 ? `${s.slice(0, 39)}…` : s}"`

/** The tasks a call marks done while work under them is still open (Org's org-enforce-todo-dependencies and its
 *  checkbox twin: a DONE that still shows [0/4] says the work isn't done). Each is refused, naming what's open, unless
 *  `all` says every one of them is done: then they are ticked and closed with it, after the call's own edits. */
function closesOpen(text: string, edits: PlanEdit[], all: boolean, tool: PlanTool): PlanEdit[] {
  const before = readPlan(text)
  const done = edits.flatMap((e, i) => (e.op === "state" && meant(before, e.state) === "done" && meant(before, find(before, e.ref)?.keyword ?? "") !== "done" ? [i] : []))
  if (!done.length) return []
  const dry = applyEdits(text, edits)
  const fix: PlanEdit[] = []
  for (const i of done) {
    const t = find(dry.plan, dry.refs[i])
    if (!t) continue
    const left = openUnder(dry.plan, t)
    if (!left.boxes.length && !left.tasks.length) continue
    if (!all) {
      const parts = [
        ...(left.boxes.length ? [`${left.boxes.length} unticked checkbox${left.boxes.length > 1 ? "es" : ""} (${left.boxes.map((b) => `${b} ${quoted(t.checks[b].text)}`).join(", ")})`] : []),
        ...(left.tasks.length ? [`${left.tasks.length} open subtask${left.tasks.length > 1 ? "s" : ""} (${left.tasks.map((k) => `${k.ref} ${quoted(k.text)}`).join(", ")})`] : []),
      ]
      const how = tool === "plan_close" ? "or close it with all: true when every one is done" : "or close it with plan_close and all: true when every one is done"
      throw new Error(`${t.ref} still has ${parts.join(" and ")}: tick what's done (plan_update check), close or move what's left, ${how}`)
    }
    const state = (edits[i] as Extract<PlanEdit, { op: "state" }>).state
    for (const k of [t, ...left.tasks]) for (const b of openUnder(dry.plan, k).boxes) fix.push({ op: "check", ref: k.ref, index: b, done: true })
    for (const k of left.tasks) fix.push({ op: "state", ref: k.ref, state })
  }
  return fix
}

/** A tool answered: plan_read gives the open tasks by id (and the whole text with full), giving any task without an id
 *  one first; an edit gives the ids it touched and the open tasks after it. */
export async function answerPlanTool(tool: PlanTool, input: Record<string, unknown>, h: PlanToolHost): Promise<unknown> {
  const file = h.file ?? "plan.org"
  if (tool === "plan_read") {
    let text = h.text()
    // a task written without an id (by hand, in Emacs) is given one first: an agent names tasks by id, never by place
    if (readPlan(text).tasks.some((t) => !t.id)) text = (await h.apply([])).text
    return { file, open: planDigest(readPlan(text), { max: 200 }), ...(input.full ? { text } : {}) }
  }
  const edits = planToolEdits(tool, input, h.who)
  // plan_close closes: a keyword of the plan's that isn't after its bar (MAKING) is plan_update's
  const close = edits[0]
  if (tool === "plan_close" && close.op === "state" && !/^(done|dropped)$/i.test(close.state) && !isClosed(readPlan(h.text()).words.get(close.state)))
    throw new Error(`state: done or dropped, or a closing keyword of the plan's; ${close.state} doesn't close (plan_update sets it)`)
  // a task marked done with work still open under it: every task named by id first, so the ids a dry run reads are the
  // ones the file keeps
  let text = h.text()
  if (edits.some((e) => e.op === "state") && readPlan(text).tasks.some((t) => !t.id)) text = (await h.apply([])).text
  const fix = closesOpen(text, edits, tool === "plan_close" && input.all === true, tool)
  const r = await h.apply([...edits, ...fix])
  return { file, ids: [...new Set(r.refs.filter(Boolean))], open: planDigest(readPlan(r.text)) }
}
