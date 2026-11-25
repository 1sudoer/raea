//! Two versions of a composition that differ only in the values of its `<Composition defaultProps>` (docs/realtime.feature
//! RT4f): a control's value written into the file, as the app writes one on release. The stage takes such a version in
//! place of opening it again (its props laid over the running world), so writing a value costs no reopen.

use oxc::allocator::Allocator;
use oxc::ast::ast::*;
use oxc::ast_visit::{Visit, walk};
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType, Span};
use serde_json::{Map, Number, Value};

/// The `defaultProps={...}` of each `<Composition>` element: its span, and its values when it is an object written out.
#[derive(Default)]
struct Defaults {
    found: Vec<(Span, Option<Map<String, Value>>)>,
}

impl<'a> Visit<'a> for Defaults {
    fn visit_jsx_opening_element(&mut self, it: &JSXOpeningElement<'a>) {
        // a capitalised name is a reference to the component (`Composition`, imported from raea)
        if matches!(&it.name, JSXElementName::IdentifierReference(n) if n.name == "Composition") {
            for a in &it.attributes {
                if let JSXAttributeItem::Attribute(a) = a
                    && matches!(&a.name, JSXAttributeName::Identifier(n) if n.name == "defaultProps")
                    && let Some(JSXAttributeValue::ExpressionContainer(c)) = &a.value
                    && let Some(e) = c.expression.as_expression()
                {
                    self.found.push((e.span(), match e.without_parentheses() {
                        Expression::ObjectExpression(o) => object(o),
                        _ => None,
                    }));
                }
            }
        }
        walk::walk_jsx_opening_element(self, it);
    }
}

/// A value written out in the file as JSON: a string, number, true/false, null, or an array or object of them. None for
/// anything worked out (a name, a call, a spread, a computed key).
fn value(e: &Expression) -> Option<Value> {
    Some(match e.without_parentheses() {
        Expression::StringLiteral(s) => Value::String(s.value.to_string()),
        Expression::TemplateLiteral(t) if t.expressions.is_empty() => Value::String(t.quasis.iter().map(|q| q.value.cooked.as_ref().map_or(q.value.raw.as_str(), |c| c.as_str())).collect()),
        Expression::NumericLiteral(n) => number(n.value)?,
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::UnaryNegation => match &u.argument {
            Expression::NumericLiteral(n) => number(-n.value)?,
            _ => return None,
        },
        Expression::BooleanLiteral(b) => Value::Bool(b.value),
        Expression::NullLiteral(_) => Value::Null,
        Expression::ArrayExpression(a) => Value::Array(a.elements.iter().map(|el| el.as_expression().and_then(value)).collect::<Option<_>>()?),
        Expression::ObjectExpression(o) => Value::Object(object(o)?),
        Expression::TSAsExpression(t) => value(&t.expression)?,
        Expression::TSSatisfiesExpression(t) => value(&t.expression)?,
        _ => return None,
    })
}

/// A number as JSON.stringify writes it (35, not 35.0), so the values compare with what the composition's JS reports.
fn number(v: f64) -> Option<Value> {
    if v.fract() == 0.0 && v.abs() < 9_007_199_254_740_992.0 {
        return Some(Value::Number(Number::from(v as i64)));
    }
    Number::from_f64(v).map(Value::Number)
}

fn object(o: &ObjectExpression) -> Option<Map<String, Value>> {
    let mut out = Map::new();
    for p in &o.properties {
        let ObjectPropertyKind::ObjectProperty(p) = p else { return None };
        if p.computed || p.method || p.kind != PropertyKind::Init {
            return None;
        }
        out.insert(p.key.static_name()?.to_string(), value(&p.value)?);
    }
    Some(out)
}

/// The one `<Composition defaultProps>` literal of a source, its span and its values; None when it has none, several, or
/// one that isn't written out as values, or doesn't parse.
fn defaults(src: &str) -> Option<(Span, Map<String, Value>)> {
    let alloc = Allocator::default();
    let ret = Parser::new(&alloc, src, SourceType::tsx()).parse();
    if ret.diagnostics.errors().next().is_some() {
        return None;
    }
    let mut d = Defaults::default();
    d.visit_program(&ret.program);
    match d.found.pop() {
        Some((span, Some(values))) if d.found.is_empty() => Some((span, values)),
        _ => None,
    }
}

/// `new`'s defaultProps when it is `old` with only the values of its one `<Composition defaultProps={{ ... }}>` changed:
/// the same text around them, the same keys, each value written out (no name or call). None for any other change, which
/// the stage opens again.
pub fn values_only(old: &str, new: &str) -> Option<Map<String, Value>> {
    let (a, was) = defaults(old)?;
    let (b, now) = defaults(new)?;
    let around = |s: &str, sp: Span| (s[..sp.start as usize].to_string(), s[sp.end as usize..].to_string());
    if around(old, a) != around(new, b) || was.len() != now.len() || !was.keys().all(|k| now.contains_key(k)) {
        return None;
    }
    Some(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FILM: &str = r##"import { Composition, registerRoot } from "raea";
import { Film } from "./film";
const W = 1920;
registerRoot(() => (
  <Composition id="mayor" component={Film} width={W} height={1080} fps={30} durationInFrames={2700}
    controls={{ warmth: { kind: "number", min: -1, max: 1 } }}
    defaultProps={{ warmth: 0.2, model: "goose", rain: false, tint: "#ff8800", shots: [1, 2], cam: { fov: 40 } }} />
));
"##;

    #[test]
    fn a_value_written_into_the_defaults_is_values_only() {
        let new = FILM.replace("warmth: 0.2", "warmth: -0.5").replace(r##""#ff8800""##, r##""#00ff88""##).replace("fov: 40", "fov: 35");
        let v = values_only(FILM, &new).unwrap();
        assert_eq!(Value::Object(v), json!({"warmth": -0.5, "model": "goose", "rain": false, "tint": "#00ff88", "shots": [1, 2], "cam": {"fov": 35}}));
        // the same file is values only too, its values as they are
        assert_eq!(values_only(FILM, FILM).unwrap()["model"], json!("goose"));
    }

    #[test]
    fn anything_else_changed_is_not() {
        // code outside the defaults
        assert!(values_only(FILM, &FILM.replace("height={1080}", "height={720}")).is_none());
        // a control's spec
        assert!(values_only(FILM, &FILM.replace("max: 1 }", "max: 2 }")).is_none());
        // a key added or taken away
        assert!(values_only(FILM, &FILM.replace("rain: false, ", "")).is_none());
        assert!(values_only(FILM, &FILM.replace("rain: false,", "rain: false, wind: 1,")).is_none());
        // a value worked out, or named
        assert!(values_only(FILM, &FILM.replace("warmth: 0.2", "warmth: W / 10")).is_none());
        assert!(values_only(FILM, &FILM.replace("warmth: 0.2", "warmth: W")).is_none());
        // a file that doesn't parse, or has no literal defaults, or two compositions with them
        assert!(values_only(FILM, &FILM.replace("registerRoot(() => (", "registerRoot(() => ((")).is_none());
        let named = FILM.replace(r###"defaultProps={{ warmth: 0.2, model: "goose", rain: false, tint: "#ff8800", shots: [1, 2], cam: { fov: 40 } }}"###, "defaultProps={PROPS}");
        assert!(values_only(&named, &named).is_none());
        let two = FILM.replace("/>\n));", "/>\n));\nconst Two = () => <Composition id=\"b\" component={Film} width={1} height={1} fps={30} durationInFrames={1} defaultProps={{ a: 1 }} />;");
        assert!(values_only(&two, &two).is_none());
    }
}
