//! What a JavaScriptCore context holds, for the heap log (crate::Upkeep, RAEA_INSPECT), and the context opened to
//! Safari's Web Inspector: both through calls the system exports without declaring them (super::spi).

use super::*;

impl Context {
    /// What the heap holds, as JSON: JavaScriptCore's own statistics (JSGetMemoryUsageStatistics, exported by the system
    /// though not in its headers: heapSize, heapCapacity, extraMemorySize, objectCount, protectedObjectCount,
    /// objectTypeCounts by class), found at run time. None where the system doesn't export it.
    pub fn heap(&self) -> Option<String> {
        type Stats = unsafe extern "C" fn(Ctx) -> Obj;
        static STATS: std::sync::OnceLock<Option<Stats>> = std::sync::OnceLock::new();
        let f = (*STATS.get_or_init(|| spi(c"JSGetMemoryUsageStatistics")))?;
        unsafe {
            let stats = f(self.c());
            if stats.is_null() {
                return None;
            }
            let json = JSValueCreateJSONString(self.c(), stats as Val, 0, null_mut());
            if json.is_null() {
                return None;
            }
            let v = JSValueMakeString(self.c(), json);
            JSStringRelease(json);
            Some(to_string(self.c(), v))
        }
    }
}

/// The context listed in Safari's Develop menu, for its Web Inspector (heap snapshots, what holds an object, the
/// console), when RAEA_INSPECT is on: JSGlobalContextSetInspectable, macOS 13.3 and later, found at run time so an older
/// system starts as before.
pub(super) fn inspectable(ctx: GCtx) {
    type Set = unsafe extern "C" fn(GCtx, bool);
    match spi::<Set>(c"JSGlobalContextSetInspectable") {
        Some(f) => unsafe { f(ctx, true) },
        None => eprintln!("raea-runtime-jsengine: this system's JavaScriptCore can't be inspected (macOS 13.3 or later)"),
    }
}
