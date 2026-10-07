// Adapted from TFormula 0.3.1 under MIT; see TFORMULA-LICENSE.
function horizontalPadding(cell, display) {
    return display ? cell.width : Math.min(1, cell.width * 0.1);
}
function verticalPadding(cell, display) {
    return display ? 0 : Math.max(1, cell.height * 0.08);
}
export function calculateFormulaGeometry(input) {
    const canvasWidth = Math.max(1, Math.round(input.columns * input.cell.width));
    const canvasHeight = Math.max(1, Math.round(input.rows * input.cell.height));
    const exPx = input.cell.height * 0.45 * input.scale;
    const naturalHeight = Math.max(1, input.naturalHeightEx * exPx);
    const naturalWidth = Math.max(1, naturalHeight * input.aspectRatio);
    const paddingX = horizontalPadding(input.cell, input.display);
    // A display region is whitespace reserved by the TUI, even when the source
    // only spanned a single row. Use its full height so tall fractions keep the
    // same glyph scale as simple equations. Inline formulas retain padding for
    // adjacent terminal text.
    const paddingY = verticalPadding(input.cell, input.display);
    const availableWidth = Math.max(1, canvasWidth - paddingX * 2);
    const availableHeight = Math.max(1, canvasHeight - paddingY * 2);
    // Never enlarge merely to fill the source rectangle; only shrink to fit.
    const fit = Math.min(1, availableWidth / naturalWidth, availableHeight / naturalHeight);
    const formulaWidth = Math.max(1, Math.round(naturalWidth * fit));
    const formulaHeight = Math.max(1, Math.round(naturalHeight * fit));
    const depthRatio = Math.max(0, Math.min(1, input.depthEx / input.naturalHeightEx));
    const scaledDepth = formulaHeight * depthRatio;
    // Terminal protocols expose cell dimensions, but not font ascent. A baseline
    // near 78% of the cell height matches the usual terminal font metrics. Honor
    // MathJax's depth below that baseline so rho, J, and subscripted symbols line
    // up with neighboring terminal text instead of centering unlike bounding boxes.
    const inlineBaseline = input.cell.height * 0.78;
    const baselineOffsetY = Math.round(inlineBaseline - (formulaHeight - scaledDepth));
    const maxOffsetY = Math.max(0, canvasHeight - formulaHeight);
    return {
        canvasWidth,
        canvasHeight,
        formulaWidth,
        formulaHeight,
        fitScale: fit,
        offsetX: input.leftAlign
            ? Math.round(paddingX)
            : Math.round((canvasWidth - formulaWidth) / 2),
        offsetY: input.display
            ? Math.round((canvasHeight - formulaHeight) / 2)
            : Math.max(0, Math.min(maxOffsetY, baselineOffsetY))
    };
}
