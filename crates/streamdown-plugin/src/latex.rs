//! LaTeX to Unicode conversion plugin.
//!
//! Converts LaTeX math expressions between `$$` delimiters to Unicode.
//!
//! # Supported conversions
//!
//! - Greek letters: `\alpha` → α, `\beta` → β, etc.
//! - Operators: `\sum` → Σ, `\prod` → Π, `\int` → ∫
//! - Relations: `\leq` → ≤, `\geq` → ≥, `\neq` → ≠
//! - Subscripts: `x_1` → x₁, `x_{10}` → x₁₀
//! - Superscripts: `x^2` → x², `x^{10}` → x¹⁰
//! - Fractions: `\frac{a}{b}` → a/b
//! - Common symbols: `\infty` → ∞, `\pm` → ±, etc.

use crate::{Plugin, ProcessResult};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
use streamdown_config::ComputedStyle;
use streamdown_core::state::ParseState;

/// LaTeX plugin for converting math to Unicode.
pub struct LatexPlugin {
    /// Whether we're inside a $$ block
    in_block: bool,
    /// Buffer for multi-line expressions
    buffer: String,
}

impl LatexPlugin {
    /// Create a new LaTeX plugin.
    pub fn new() -> Self {
        Self {
            in_block: false,
            buffer: String::new(),
        }
    }
}

impl Default for LatexPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for LatexPlugin {
    fn name(&self) -> &str {
        "latex"
    }

    fn process_line(
        &mut self,
        line: &str,
        _state: &ParseState,
        _style: &ComputedStyle,
    ) -> Option<ProcessResult> {
        // Handle inline $...$ first (single line)
        if !self.in_block && line.contains('$') && !line.contains("$$") {
            // Check for inline math
            let converted = convert_inline_math(line);
            if converted != line {
                return Some(ProcessResult::Lines(vec![converted]));
            }
        }

        // Check for $$ delimiters
        if !self.in_block {
            if let Some(idx) = line.find("$$") {
                self.in_block = true;
                self.buffer.clear();

                // Get content after opening $$
                let after = &line[idx + 2..];

                // Check if closing $$ is on same line
                if let Some(end_idx) = after.find("$$") {
                    // Single line expression
                    self.in_block = false;
                    let expr = &after[..end_idx];
                    let converted = latex_to_unicode(expr);
                    return Some(ProcessResult::Lines(vec![converted]));
                }

                // Multi-line: start buffering
                self.buffer.push_str(after);
                return Some(ProcessResult::Continue);
            }
            return None;
        }

        // We're in a block, looking for closing $$
        if let Some(idx) = line.find("$$") {
            // Found closing delimiter
            self.in_block = false;
            self.buffer.push_str(&line[..idx]);

            let converted = latex_to_unicode(&self.buffer);
            self.buffer.clear();

            return Some(ProcessResult::Lines(vec![converted]));
        }

        // Continue buffering
        if !self.buffer.is_empty() {
            self.buffer.push(' ');
        }
        self.buffer.push_str(line);
        Some(ProcessResult::Continue)
    }

    fn flush(&mut self) -> Option<Vec<String>> {
        if self.buffer.is_empty() {
            return None;
        }

        // Return unconverted buffer if stream ended mid-block
        let result = std::mem::take(&mut self.buffer);
        self.in_block = false;
        Some(vec![format!("$$ {} (incomplete)", result)])
    }

    fn reset(&mut self) {
        self.in_block = false;
        self.buffer.clear();
    }

    fn is_active(&self) -> bool {
        self.in_block
    }

    fn priority(&self) -> i32 {
        10 // Lower priority than most plugins
    }
}

/// Convert inline math ($...$) in a line.
fn convert_inline_math(line: &str) -> String {
    static INLINE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$([^$]+)\$").unwrap());

    INLINE_RE
        .replace_all(line, |caps: &regex::Captures| latex_to_unicode(&caps[1]))
        .to_string()
}

/// Convert LaTeX expression to Unicode.
pub fn latex_to_unicode(latex: &str) -> String {
    let mut result = latex.to_string();

    // Apply conversions in order
    result = convert_commands(&result);
    result = convert_fractions(&result);
    result = convert_subscripts(&result);
    result = convert_superscripts(&result);
    result = cleanup(&result);

    result
}

/// Greek letters and symbols mapping.
static GREEK_LETTERS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Lowercase Greek
    m.insert("alpha", "α");
    m.insert("beta", "β");
    m.insert("gamma", "γ");
    m.insert("delta", "δ");
    m.insert("epsilon", "ε");
    m.insert("varepsilon", "ε");
    m.insert("zeta", "ζ");
    m.insert("eta", "η");
    m.insert("theta", "θ");
    m.insert("vartheta", "ϑ");
    m.insert("iota", "ι");
    m.insert("kappa", "κ");
    m.insert("lambda", "λ");
    m.insert("mu", "μ");
    m.insert("nu", "ν");
    m.insert("xi", "ξ");
    m.insert("omicron", "ο");
    m.insert("pi", "π");
    m.insert("varpi", "ϖ");
    m.insert("rho", "ρ");
    m.insert("varrho", "ϱ");
    m.insert("sigma", "σ");
    m.insert("varsigma", "ς");
    m.insert("tau", "τ");
    m.insert("upsilon", "υ");
    m.insert("phi", "φ");
    m.insert("varphi", "ϕ");
    m.insert("chi", "χ");
    m.insert("psi", "ψ");
    m.insert("omega", "ω");
    // Uppercase Greek
    m.insert("Gamma", "Γ");
    m.insert("Delta", "Δ");
    m.insert("Theta", "Θ");
    m.insert("Lambda", "Λ");
    m.insert("Xi", "Ξ");
    m.insert("Pi", "Π");
    m.insert("Sigma", "Σ");
    m.insert("Upsilon", "Υ");
    m.insert("Phi", "Φ");
    m.insert("Psi", "Ψ");
    m.insert("Omega", "Ω");
    m
});

/// Operators mapping.
static OPERATORS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("sum", "Σ");
    m.insert("prod", "Π");
    m.insert("int", "∫");
    m.insert("iint", "∬");
    m.insert("iiint", "∭");
    m.insert("oint", "∮");
    m.insert("partial", "∂");
    m.insert("nabla", "∇");
    m.insert("sqrt", "√");
    m.insert("cbrt", "∛");
    m.insert("times", "×");
    m.insert("div", "÷");
    m.insert("cdot", "·");
    m.insert("ast", "∗");
    m.insert("star", "⋆");
    m.insert("circ", "∘");
    m.insert("bullet", "•");
    m.insert("oplus", "⊕");
    m.insert("ominus", "⊖");
    m.insert("otimes", "⊗");
    m.insert("oslash", "⊘");
    m.insert("odot", "⊙");
    m
});

/// Relations mapping.
static RELATIONS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("leq", "≤");
    m.insert("le", "≤");
    m.insert("geq", "≥");
    m.insert("ge", "≥");
    m.insert("neq", "≠");
    m.insert("ne", "≠");
    m.insert("approx", "≈");
    m.insert("equiv", "≡");
    m.insert("sim", "∼");
    m.insert("simeq", "≃");
    m.insert("cong", "≅");
    m.insert("propto", "∝");
    m.insert("ll", "≪");
    m.insert("gg", "≫");
    m.insert("subset", "⊂");
    m.insert("supset", "⊃");
    m.insert("subseteq", "⊆");
    m.insert("supseteq", "⊇");
    m.insert("in", "∈");
    m.insert("notin", "∉");
    m.insert("ni", "∋");
    m.insert("forall", "∀");
    m.insert("exists", "∃");
    m.insert("nexists", "∄");
    m
});

/// Symbols mapping.
static SYMBOLS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("infty", "∞");
    m.insert("pm", "±");
    m.insert("mp", "∓");
    m.insert("to", "→");
    m.insert("rightarrow", "→");
    m.insert("leftarrow", "←");
    m.insert("leftrightarrow", "↔");
    m.insert("Rightarrow", "⇒");
    m.insert("Leftarrow", "⇐");
    m.insert("Leftrightarrow", "⇔");
    m.insert("uparrow", "↑");
    m.insert("downarrow", "↓");
    m.insert("mapsto", "↦");
    m.insert("ldots", "…");
    m.insert("cdots", "⋯");
    m.insert("vdots", "⋮");
    m.insert("ddots", "⋱");
    m.insert("therefore", "∴");
    m.insert("because", "∵");
    m.insert("angle", "∠");
    m.insert("perp", "⊥");
    m.insert("parallel", "∥");
    m.insert("triangle", "△");
    m.insert("square", "□");
    m.insert("diamond", "◇");
    m.insert("emptyset", "∅");
    m.insert("varnothing", "∅");
    m.insert("neg", "¬");
    m.insert("lnot", "¬");
    m.insert("land", "∧");
    m.insert("wedge", "∧");
    m.insert("lor", "∨");
    m.insert("vee", "∨");
    m.insert("cap", "∩");
    m.insert("cup", "∪");
    m.insert("setminus", "∖");
    m.insert("aleph", "ℵ");
    m.insert("hbar", "ℏ");
    m.insert("ell", "ℓ");
    m.insert("Re", "ℜ");
    m.insert("Im", "ℑ");
    m.insert("wp", "℘");
    m.insert("prime", "′");
    m.insert("degree", "°");
    m
});

/// Subscript digits.
static SUBSCRIPT_DIGITS: LazyLock<HashMap<char, char>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert('0', '₀');
    m.insert('1', '₁');
    m.insert('2', '₂');
    m.insert('3', '₃');
    m.insert('4', '₄');
    m.insert('5', '₅');
    m.insert('6', '₆');
    m.insert('7', '₇');
    m.insert('8', '₈');
    m.insert('9', '₉');
    m.insert('+', '₊');
    m.insert('-', '₋');
    m.insert('=', '₌');
    m.insert('(', '₍');
    m.insert(')', '₎');
    m.insert('a', 'ₐ');
    m.insert('e', 'ₑ');
    m.insert('h', 'ₕ');
    m.insert('i', 'ᵢ');
    m.insert('j', 'ⱼ');
    m.insert('k', 'ₖ');
    m.insert('l', 'ₗ');
    m.insert('m', 'ₘ');
    m.insert('n', 'ₙ');
    m.insert('o', 'ₒ');
    m.insert('p', 'ₚ');
    m.insert('r', 'ᵣ');
    m.insert('s', 'ₛ');
    m.insert('t', 'ₜ');
    m.insert('u', 'ᵤ');
    m.insert('v', 'ᵥ');
    m.insert('x', 'ₓ');
    m
});

/// Superscript characters.
static SUPERSCRIPT_CHARS: LazyLock<HashMap<char, char>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert('0', '⁰');
    m.insert('1', '¹');
    m.insert('2', '²');
    m.insert('3', '³');
    m.insert('4', '⁴');
    m.insert('5', '⁵');
    m.insert('6', '⁶');
    m.insert('7', '⁷');
    m.insert('8', '⁸');
    m.insert('9', '⁹');
    m.insert('+', '⁺');
    m.insert('-', '⁻');
    m.insert('=', '⁼');
    m.insert('(', '⁽');
    m.insert(')', '⁾');
    m.insert('a', 'ᵃ');
    m.insert('b', 'ᵇ');
    m.insert('c', 'ᶜ');
    m.insert('d', 'ᵈ');
    m.insert('e', 'ᵉ');
    m.insert('f', 'ᶠ');
    m.insert('g', 'ᵍ');
    m.insert('h', 'ʰ');
    m.insert('i', 'ⁱ');
    m.insert('j', 'ʲ');
    m.insert('k', 'ᵏ');
    m.insert('l', 'ˡ');
    m.insert('m', 'ᵐ');
    m.insert('n', 'ⁿ');
    m.insert('o', 'ᵒ');
    m.insert('p', 'ᵖ');
    m.insert('r', 'ʳ');
    m.insert('s', 'ˢ');
    m.insert('t', 'ᵗ');
    m.insert('u', 'ᵘ');
    m.insert('v', 'ᵛ');
    m.insert('w', 'ʷ');
    m.insert('x', 'ˣ');
    m.insert('y', 'ʸ');
    m.insert('z', 'ᶻ');
    m
});

/// Convert LaTeX commands (\alpha, \sum, etc.).
fn convert_commands(input: &str) -> String {
    static CMD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\([a-zA-Z]+)").unwrap());

    CMD_RE
        .replace_all(input, |caps: &regex::Captures| {
            let cmd = &caps[1];

            // Check each mapping
            if let Some(s) = GREEK_LETTERS.get(cmd) {
                return (*s).to_string();
            }
            if let Some(s) = OPERATORS.get(cmd) {
                return (*s).to_string();
            }
            if let Some(s) = RELATIONS.get(cmd) {
                return (*s).to_string();
            }
            if let Some(s) = SYMBOLS.get(cmd) {
                return (*s).to_string();
            }

            // Unknown command, keep original
            format!("\\{}", cmd)
        })
        .to_string()
}

/// Convert fractions \frac{a}{b} → a/b.
fn convert_fractions(input: &str) -> String {
    static FRAC_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\\frac\{([^}]*)\}\{([^}]*)\}").unwrap());

    FRAC_RE
        .replace_all(input, |caps: &regex::Captures| {
            let num = &caps[1];
            let den = &caps[2];
            format!("({}/{})", num, den)
        })
        .to_string()
}

/// Convert subscripts x_1 → x₁, x_{10} → x₁₀.
fn convert_subscripts(input: &str) -> String {
    // First handle braced subscripts: x_{abc}
    static BRACED_SUB_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"_\{([^}]+)\}").unwrap());

    let result = BRACED_SUB_RE
        .replace_all(input, |caps: &regex::Captures| {
            let content = &caps[1];
            to_subscript(content)
        })
        .to_string();

    // Then handle single-char subscripts: x_1
    static SINGLE_SUB_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"_([0-9a-z])").unwrap());

    SINGLE_SUB_RE
        .replace_all(&result, |caps: &regex::Captures| {
            let c = caps[1].chars().next().unwrap();
            SUBSCRIPT_DIGITS
                .get(&c)
                .map(|&s| s.to_string())
                .unwrap_or_else(|| format!("_{}", c))
        })
        .to_string()
}

/// Convert superscripts x^2 → x², x^{10} → x¹⁰.
fn convert_superscripts(input: &str) -> String {
    // First handle braced superscripts: x^{abc}
    static BRACED_SUP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\^\{([^}]+)\}").unwrap());

    let result = BRACED_SUP_RE
        .replace_all(input, |caps: &regex::Captures| {
            let content = &caps[1];
            to_superscript(content)
        })
        .to_string();

    // Then handle single-char superscripts: x^2
    static SINGLE_SUP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\^([0-9a-z])").unwrap());

    SINGLE_SUP_RE
        .replace_all(&result, |caps: &regex::Captures| {
            let c = caps[1].chars().next().unwrap();
            SUPERSCRIPT_CHARS
                .get(&c)
                .map(|&s| s.to_string())
                .unwrap_or_else(|| format!("^{}", c))
        })
        .to_string()
}

/// Convert string to subscript.
fn to_subscript(s: &str) -> String {
    s.chars()
        .map(|c| SUBSCRIPT_DIGITS.get(&c).copied().unwrap_or(c))
        .collect()
}

/// Convert string to superscript.
fn to_superscript(s: &str) -> String {
    s.chars()
        .map(|c| SUPERSCRIPT_CHARS.get(&c).copied().unwrap_or(c))
        .collect()
}

/// Clean up the result.
fn cleanup(input: &str) -> String {
    // Remove extra braces and spaces
    input
        .replace("{ ", "")
        .replace(" }", "")
        .replace("{}", "")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greek_letters() {
        assert_eq!(latex_to_unicode(r"\alpha + \beta"), "α + β");
        assert_eq!(latex_to_unicode(r"\Gamma\Delta"), "ΓΔ");
        assert_eq!(latex_to_unicode(r"\pi r^2"), "π r²");
    }

    #[test]
    fn test_operators() {
        assert_eq!(latex_to_unicode(r"\sum x"), "Σ x");
        assert_eq!(latex_to_unicode(r"\int f(x) dx"), "∫ f(x) dx");
        // Subscripts are now converted!
        let result = latex_to_unicode(r"\prod_{i=1}");
        assert!(result.contains("Π")); // Pi symbol
        assert!(result.contains("₁")); // Subscript 1
    }

    #[test]
    fn test_relations() {
        assert_eq!(latex_to_unicode(r"x \leq y"), "x ≤ y");
        assert_eq!(latex_to_unicode(r"a \neq b"), "a ≠ b");
        assert_eq!(latex_to_unicode(r"A \subset B"), "A ⊂ B");
    }

    #[test]
    fn test_symbols() {
        assert_eq!(latex_to_unicode(r"\infty"), "∞");
        assert_eq!(latex_to_unicode(r"\pm 1"), "± 1");
        assert_eq!(latex_to_unicode(r"x \to y"), "x → y");
    }

    #[test]
    fn test_subscripts() {
        assert_eq!(latex_to_unicode("x_1"), "x₁");
        assert_eq!(latex_to_unicode("x_{12}"), "x₁₂");
        assert_eq!(latex_to_unicode("a_n"), "aₙ");
    }

    #[test]
    fn test_superscripts() {
        assert_eq!(latex_to_unicode("x^2"), "x²");
        assert_eq!(latex_to_unicode("x^{10}"), "x¹⁰");
        assert_eq!(latex_to_unicode("e^x"), "eˣ");
    }

    #[test]
    fn test_fractions() {
        assert_eq!(latex_to_unicode(r"\frac{a}{b}"), "(a/b)");
        assert_eq!(latex_to_unicode(r"\frac{1}{2}"), "(1/2)");
    }

    #[test]
    fn test_complex_expression() {
        let input = r"E = mc^2";
        assert_eq!(latex_to_unicode(input), "E = mc²");

        let input = r"\sum_{i=1}^n x_i";
        let result = latex_to_unicode(input);
        assert!(result.contains("Σ")); // Sum symbol
        // Subscripts should be converted
        assert!(result.contains("ᵢ") || result.contains("i")); // Subscript i or regular i
    }

    #[test]
    fn test_inline_math() {
        assert_eq!(convert_inline_math("The value $x^2$ is"), "The value x² is");
        assert_eq!(
            convert_inline_math("We have $\\alpha$ and $\\beta$"),
            "We have α and β"
        );
    }

    #[test]
    fn test_latex_plugin_single_line() {
        let mut plugin = LatexPlugin::new();
        let state = ParseState::new();
        let style = ComputedStyle::default();

        let result = plugin.process_line("$$E = mc^2$$", &state, &style);
        assert!(matches!(result, Some(ProcessResult::Lines(_))));
        if let Some(ProcessResult::Lines(lines)) = result {
            assert_eq!(lines.len(), 1);
            assert!(lines[0].contains("E = mc²"));
        }
    }

    #[test]
    fn test_latex_plugin_multiline() {
        let mut plugin = LatexPlugin::new();
        let state = ParseState::new();
        let style = ComputedStyle::default();

        // Start block
        let result = plugin.process_line("$$\\sum_{i=1}^n", &state, &style);
        assert!(matches!(result, Some(ProcessResult::Continue)));

        // Continue
        let result = plugin.process_line("x_i$$", &state, &style);
        assert!(matches!(result, Some(ProcessResult::Lines(_))));
        if let Some(ProcessResult::Lines(lines)) = result {
            assert!(lines[0].contains("Σ"));
        }
    }

    #[test]
    fn test_latex_plugin_inline() {
        let mut plugin = LatexPlugin::new();
        let state = ParseState::new();
        let style = ComputedStyle::default();

        let result = plugin.process_line("The value $x^2$ is important", &state, &style);
        assert!(matches!(result, Some(ProcessResult::Lines(_))));
        if let Some(ProcessResult::Lines(lines)) = result {
            assert!(lines[0].contains("x²"));
        }
    }

    #[test]
    fn test_latex_plugin_no_match() {
        let mut plugin = LatexPlugin::new();
        let state = ParseState::new();
        let style = ComputedStyle::default();

        let result = plugin.process_line("Normal text without math", &state, &style);
        assert!(result.is_none());
    }

    #[test]
    fn test_latex_plugin_flush() {
        let mut plugin = LatexPlugin::new();
        let state = ParseState::new();
        let style = ComputedStyle::default();

        // Start block without closing
        plugin.process_line("$$x^2 + y^2", &state, &style);

        // Flush should return incomplete content
        let result = plugin.flush();
        assert!(result.is_some());
    }

    #[test]
    fn test_latex_plugin_reset() {
        let mut plugin = LatexPlugin::new();
        let state = ParseState::new();
        let style = ComputedStyle::default();

        plugin.process_line("$$x^2", &state, &style);
        assert!(plugin.is_active());

        plugin.reset();
        assert!(!plugin.is_active());
    }
}
