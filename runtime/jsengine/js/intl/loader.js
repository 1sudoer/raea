// QuickJS's Intl (src/quickjs.rs runs this first in every context): JavaScriptCore, the Mac's engine, has Intl, and
// QuickJS, Windows' and Linux's, has none, so a composition that formatted a number threw there (#169). FormatJS's
// polyfills are the core of intl.pack (build.ts), unpacked and run the first time anything reads Intl or calls a
// toLocale* method, so a script that never formats pays nothing; a locale's data runs the first time something asks
// for that locale. FormatJS has no Collator: the small one here sorts by letter, then accent, then case (lower first),
// as ICU's root order does for words, and needs none of the core.
(() => {
  const g = globalThis;
  if (typeof g.Intl !== "undefined") return;
  const chunk = g.__raea_intl_chunk;
  const zone = g.__raea_intl_zone;
  delete g.__raea_intl_chunk;
  delete g.__raea_intl_zone;
  const run = (src) => (0, eval)(src);
  let intl;
  let packed = [];
  const loaded = new Set(["en"]);
  // the pack's locale for a tag: itself, else its language (zh-TW, zh-HK and zh-MO are written as zh-Hant)
  const packedFor = (tag) => {
    const t = String(tag);
    const traditional = /^zh-(Hant|TW|HK|MO)\b/i.test(t) ? "zh-Hant" : null;
    for (const c of [t, traditional, t.split("-")[0]]) {
      const hit = c && packed.find((p) => p.toLowerCase() === c.toLowerCase());
      if (hit) return hit;
    }
    return null;
  };
  const need = (locales) => {
    boot();
    const tags = locales === undefined || locales === null ? [] : typeof locales === "string" || typeof locales[Symbol.iterator] !== "function" ? [locales] : [...locales];
    for (const tag of tags) {
      const p = packedFor(tag);
      if (p && !loaded.has(p)) {
        loaded.add(p);
        run(chunk(p));
      }
    }
  };
  // a constructor that runs its locales' data first: the same prototype (instanceof holds), and its statics
  const wrap = (Native) => {
    function Intl(locales, options) {
      need(locales);
      return new.target ? Reflect.construct(Native, [locales, options], new.target === Intl ? Native : new.target) : Native(locales, options);
    }
    Object.setPrototypeOf(Intl, Native);
    Object.defineProperty(Intl, "name", { value: Native.name });
    Intl.prototype = Native.prototype;
    if (Native.supportedLocalesOf) Intl.supportedLocalesOf = (locales, options) => (need(locales), Native.supportedLocalesOf(locales, options));
    return Intl;
  };

  // the toLocale* methods: until the core runs, each is one of these, which runs it and then FormatJS's own
  // (impl); whatever wraps them later (raea-runtime-js's prelude keeps one NumberFormat per locale) is kept
  const METHODS = [[Number.prototype, "toLocaleString"], [Date.prototype, "toLocaleString"], [Date.prototype, "toLocaleDateString"], [Date.prototype, "toLocaleTimeString"]];
  if (typeof BigInt !== "undefined") METHODS.push([BigInt.prototype, "toLocaleString"]);
  const impl = METHODS.map(([proto, name]) => proto[name]);
  METHODS.forEach(([proto, name], i) => {
    const mine = { [name](locales, options) {
      need(locales);
      return impl[i].call(this, locales, options);
    } }[name];
    Object.defineProperty(proto, name, { value: mine, writable: true, configurable: true });
  });

  let booting = false;
  const boot = () => {
    if (intl) return intl;
    // the core asking for Intl while it runs gets what it has made so far, not a second core
    if (booting) return g.Intl;
    booting = true;
    const before = METHODS.map(([proto, name]) => proto[name]);
    delete g.Intl;
    run(chunk("core"));
    intl = g.Intl;
    packed = JSON.parse(chunk("index"));
    METHODS.forEach(([proto, name], i) => {
      if (proto[name] !== before[i]) impl[i] = proto[name];
      Object.defineProperty(proto, name, { value: before[i], writable: true, configurable: true });
    });
    if (zone) {
      try {
        intl.DateTimeFormat.__setDefaultTimeZone(zone);
      } catch {
        // a zone outside the common ones in the pack: UTC
      }
    }
    const put = (k, v) => Object.defineProperty(intl, k, { value: v, writable: true, configurable: true });
    for (const k of ["DateTimeFormat", "NumberFormat", "PluralRules", "RelativeTimeFormat", "ListFormat"]) if (intl[k]) put(k, wrap(intl[k]));
    put("Collator", Collator);
    return intl;
  };

  const plain = (s) => s.normalize("NFD").replace(/[̀-ͯ]/g, "");
  const cmp = (x, y) => (x < y ? -1 : x > y ? 1 : 0);
  // numbers in words by their value: "item 2" before "item 10"
  const nat = (x, y) => {
    const a = x.match(/\d+|\D+/g) ?? [];
    const b = y.match(/\d+|\D+/g) ?? [];
    for (let i = 0; i < Math.min(a.length, b.length); i++) {
      const d = /^\d/.test(a[i]) && /^\d/.test(b[i]) ? Number(a[i]) - Number(b[i]) : cmp(a[i], b[i]);
      if (d) return Math.sign(d);
    }
    return Math.sign(a.length - b.length);
  };
  class Collator {
    constructor(locales, o = {}) {
      const tag = (typeof locales === "string" ? locales : locales?.[0]) ?? "en";
      this.o = { locale: String(tag), usage: o.usage ?? "sort", sensitivity: o.sensitivity ?? "variant", ignorePunctuation: !!o.ignorePunctuation, collation: "default", numeric: !!o.numeric, caseFirst: o.caseFirst ?? "false" };
      const { sensitivity: s, caseFirst } = this.o;
      const by = this.o.numeric ? nat : cmp;
      const drop = this.o.ignorePunctuation ? (x) => x.replace(/[\p{P}\s]/gu, "") : (x) => x;
      this.compare = (a, b) => {
        const x = drop(String(a));
        const y = drop(String(b));
        const letters = by(plain(x).toLowerCase(), plain(y).toLowerCase());
        if (letters || s === "base") return letters;
        if (s !== "case") {
          const accents = by(x.toLowerCase(), y.toLowerCase());
          if (accents || s === "accent") return accents;
        }
        // "case": a and A differ, a and á don't
        const cased = s === "case" ? by(plain(x), plain(y)) : by(x, y);
        return caseFirst === "upper" ? cased : -cased;
      };
    }
    resolvedOptions() {
      return { ...this.o };
    }
    static supportedLocalesOf(locales) {
      return boot().getCanonicalLocales(locales ?? []);
    }
  }
  const collators = new Map();
  Object.defineProperty(String.prototype, "localeCompare", {
    value: function localeCompare(that, locales, options) {
      const key = options === undefined && (locales === undefined || typeof locales === "string") ? (locales ?? "") : null;
      let c = key === null ? undefined : collators.get(key);
      if (!c) {
        c = new Collator(locales, options);
        if (key !== null && collators.size < 32) collators.set(key, c);
      }
      return c.compare(String(this), String(that));
    },
    writable: true,
    configurable: true,
  });
  Object.defineProperty(g, "Intl", { get: boot, set: (v) => Object.defineProperty(g, "Intl", { value: v, writable: true, configurable: true }), configurable: true });
})();
