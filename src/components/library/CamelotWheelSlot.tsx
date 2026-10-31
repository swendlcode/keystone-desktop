import { WheelPanel } from '../wheel/WheelPanel';

/**
 * The sidebar's wheel slot.
 *
 * Kept as its own name because `Sidebar` imports it from here; the wheel
 * itself lives under `components/wheel/` with its geometry and its segment.
 */
export function CamelotWheelSlot() {
  return <WheelPanel />;
}
