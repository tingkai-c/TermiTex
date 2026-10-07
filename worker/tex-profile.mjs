// Adapted from TFormula 0.3.1 under MIT; see TFORMULA-LICENSE.
export const SCIENTIFIC_TEX_PACKAGES = [
    "mhchem",
    "physics",
    "mathtools",
    "cancel",
    "centernot",
    "upgreek",
    "units",
    "gensymb",
    "cases",
    "extpfeil",
    "boldsymbol",
    "enclose",
    "configmacros"
];
/** A conservative subset of ubiquitous siunitx syntax used by CLI agents. */
export const SCIENTIFIC_TEX_MACROS = {
    SI: ["#1\\,\\mathrm{#2}", 2],
    si: ["\\mathrm{#1}", 1],
    unit: ["\\mathrm{#1}", 1],
    metre: "\\mathrm{m}",
    meter: "\\mathrm{m}",
    second: "\\mathrm{s}",
    minute: "\\mathrm{min}",
    hour: "\\mathrm{h}",
    day: "\\mathrm{d}",
    gram: "\\mathrm{g}",
    kilogram: "\\mathrm{kg}",
    mole: "\\mathrm{mol}",
    kelvin: "\\mathrm{K}",
    ampere: "\\mathrm{A}",
    candela: "\\mathrm{cd}",
    litre: "\\mathrm{L}",
    liter: "\\mathrm{L}",
    hertz: "\\mathrm{Hz}",
    newton: "\\mathrm{N}",
    pascal: "\\mathrm{Pa}",
    joule: "\\mathrm{J}",
    watt: "\\mathrm{W}",
    coulomb: "\\mathrm{C}",
    volt: "\\mathrm{V}",
    farad: "\\mathrm{F}",
    tesla: "\\mathrm{T}",
    weber: "\\mathrm{Wb}",
    henry: "\\mathrm{H}",
    siemens: "\\mathrm{S}",
    lumen: "\\mathrm{lm}",
    lux: "\\mathrm{lx}",
    becquerel: "\\mathrm{Bq}",
    gray: "\\mathrm{Gy}",
    sievert: "\\mathrm{Sv}",
    katal: "\\mathrm{kat}",
    electronvolt: "\\mathrm{eV}",
    radian: "\\mathrm{rad}",
    steradian: "\\mathrm{sr}",
    milli: "\\mathrm{m}",
    nano: "\\mathrm{n}",
    pico: "\\mathrm{p}",
    kilo: "\\mathrm{k}",
    mega: "\\mathrm{M}",
    giga: "\\mathrm{G}",
    tera: "\\mathrm{T}",
    percent: "\\%",
    degreeCelsius: "{}^{\\circ}\\mathrm{C}",
    ohm: "\\Omega",
    per: "/",
    squared: "^{2}",
    cubed: "^{3}"
};
function bracedGroupAt(source, start) {
    if (source[start] !== "{")
        return undefined;
    let depth = 0;
    for (let index = start; index < source.length; index += 1) {
        if (source[index] === "\\") {
            index += 1;
            continue;
        }
        if (source[index] === "{")
            depth += 1;
        else if (source[index] === "}") {
            depth -= 1;
            if (depth === 0)
                return { end: index + 1, source: source.slice(start, index + 1) };
        }
    }
    return undefined;
}
/**
 * siunitx v3 reuses `\qty`, which is also defined by the physics package.
 * Only its unambiguous two-braced-argument form is rewritten; physics forms
 * such as `\qty(x)` and `\qty{\frac{a}{b}}` remain byte-for-byte unchanged.
 */
function normalizeSiunitxQuantities(latex) {
    let output = "";
    let cursor = 0;
    const commands = latex.matchAll(/\\qty(?![A-Za-z])/gu);
    for (const match of commands) {
        const start = match.index;
        if (start < cursor)
            continue;
        let argumentStart = start + match[0].length;
        while (/\s/u.test(latex[argumentStart] ?? ""))
            argumentStart += 1;
        const value = bracedGroupAt(latex, argumentStart);
        if (!value)
            continue;
        let unitStart = value.end;
        while (/\s/u.test(latex[unitStart] ?? ""))
            unitStart += 1;
        const unit = bracedGroupAt(latex, unitStart);
        if (!unit)
            continue;
        output += `${latex.slice(cursor, start)}\\SI${value.source}${unit.source}`;
        cursor = unit.end;
    }
    return cursor === 0 ? latex : output + latex.slice(cursor);
}
export function normalizeLatexForRendering(latex) {
    // Rendering must not algebraically rewrite valid TeX. In particular,
    // changing x^1/\sqrt{y} into x^\frac{1}{\sqrt{y}} changes its meaning.
    // `\textcircled` is a presentational LaTeX command which MathJax 4 does not
    // implement. Map its simple form to MathJax's equivalent enclosure while
    // preserving the enclosed expression exactly.
    return normalizeSiunitxQuantities(latex)
        .replace(/\\text\s*\{\s*\\textcircled\s*\{([^{}]*)\}\s*\}/gu, (_match, contents) => `\\enclose{circle}{${contents}}`)
        .replace(/\\textcircled\s*\{([^{}]*)\}/gu, (_match, contents) => `\\enclose{circle}{${contents}}`);
}
