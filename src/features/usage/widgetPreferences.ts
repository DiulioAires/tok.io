import type { WidgetPosition } from "./types";

type DisplayBounds = {
  position: { x: number; y: number };
  size: { width: number; height: number };
  scaleFactor: number;
};

export function widgetPositionFitsDisplay(
  position: WidgetPosition,
  logicalSize: { width: number; height: number },
  display: DisplayBounds,
) {
  const width = Math.round(logicalSize.width * display.scaleFactor);
  const height = Math.round(logicalSize.height * display.scaleFactor);
  return position.x >= display.position.x
    && position.y >= display.position.y
    && position.x + width <= display.position.x + display.size.width
    && position.y + height <= display.position.y + display.size.height;
}
