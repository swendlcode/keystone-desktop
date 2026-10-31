import {
  SETTINGS_SECTIONS,
  useSettingsStore,
  type SettingsSection,
} from '../../stores/settingsStore';

/**
 * The left rail of SPEC.md §3, in the spec's order.
 *
 * The order is the spec's, not alphabetical or grouped: Personalize and Send
 * Feedback first, then the four panes that write to files, then the four that
 * only change what is displayed, then About. A user following a forum post
 * counts down the list.
 */
export function SettingsRail() {
  const section = useSettingsStore((s) => s.section);
  const setSection = useSettingsStore((s) => s.setSection);

  return (
    <nav
      aria-label="Settings"
      className="flex w-48 shrink-0 flex-col gap-0.5 border-r border-gray-700 bg-gray-900 p-2"
    >
      {SETTINGS_SECTIONS.map((s) => (
        <RailItem
          key={s.id}
          id={s.id}
          label={s.label}
          active={section === s.id}
          onSelect={setSection}
        />
      ))}
    </nav>
  );
}

function RailItem({
  id,
  label,
  active,
  onSelect,
}: {
  id: SettingsSection;
  label: string;
  active: boolean;
  onSelect: (id: SettingsSection) => void;
}) {
  return (
    <button
      type="button"
      aria-current={active ? 'page' : undefined}
      onClick={() => onSelect(id)}
      className={`rounded-md px-3 py-2 text-left text-xs transition-colors ${
        active
          ? 'bg-keystone-lime/15 font-semibold text-keystone-lime'
          : 'text-gray-300 hover:bg-gray-800 hover:text-keystone-white'
      }`}
    >
      {label}
    </button>
  );
}
