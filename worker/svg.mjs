// Adapted from TFormula 0.3.1 under MIT; see TFORMULA-LICENSE.
function parseLength(value) {
    const match = value?.match(/^((?:\d+(?:\.\d*)?|\.\d+))(ex|em|px)?$/u);
    if (!match)
        return undefined;
    const parsed = Number(match[1]);
    return Number.isFinite(parsed) && parsed > 0
        ? { value: parsed, unit: match[2] ?? "px" }
        : undefined;
}
export function readSvgDimensions(svg) {
    const width = parseLength(svg.match(/\bwidth="([^"]+)"/u)?.[1]);
    const height = parseLength(svg.match(/\bheight="([^"]+)"/u)?.[1]);
    const viewBox = svg.match(/\bviewBox="[^\s]+\s+[^\s]+\s+([\d.]+)\s+([\d.]+)"/u);
    const viewBoxRatio = viewBox ? Number(viewBox[1]) / Number(viewBox[2]) : 1;
    const fallbackRatio = Number.isFinite(viewBoxRatio) && viewBoxRatio > 0 ? viewBoxRatio : 1;
    const unitToEx = (length) => {
        if (length.unit === "ex")
            return length.value;
        if (length.unit === "em")
            return length.value * 2;
        return length.value / 8;
    };
    const parsedHeightEx = height
        ? unitToEx(height)
        : width
            ? unitToEx(width) / fallbackRatio
            : 1.8;
    const parsedWidthEx = width
        ? unitToEx(width)
        : parsedHeightEx * fallbackRatio;
    const verticalAlign = svg.match(/vertical-align:\s*(-?[\d.]+)(ex|em|px)?/u);
    const parsedVerticalAlignEx = verticalAlign
        ? unitToEx({ value: Number(verticalAlign[1]), unit: verticalAlign[2] ?? "px" })
        : 0;
    const verticalAlignEx = Number.isFinite(parsedVerticalAlignEx) ? parsedVerticalAlignEx : 0;
    const aspectRatio = parsedWidthEx / parsedHeightEx;
    return {
        aspectRatio: Number.isFinite(aspectRatio) && aspectRatio > 0 ? aspectRatio : fallbackRatio,
        heightEx: Number.isFinite(parsedHeightEx) && parsedHeightEx > 0 ? parsedHeightEx : 1.8,
        depthEx: Math.max(0, -verticalAlignEx)
    };
}
