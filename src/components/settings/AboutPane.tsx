import { Button } from '../ui/Button';
import { Heart, Logo } from '../ui/icons';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { openExternal, useAppVersion, useUpdateCheck } from './useAppInfo';

const COFFEE = 'https://www.buymeacoffee.com/swendl';

export function AboutPane() {
  const { data: version } = useAppVersion();
  const { state, check, install } = useUpdateCheck();

  return (
    <SettingSection title="About">
      <SettingGroup>
        <Logo size={22} />
        <p className="mono mt-2 text-[11px] text-gray-400">
          Version {version ?? '—'}
        </p>
        <p className="mono text-[11px] text-gray-500">
          Tauri 2 · React 18 · Rust · SQLite
        </p>
        <SettingNote>
          Harmonic mixing analysis. Everything runs on this machine.
        </SettingNote>
      </SettingGroup>

      <SettingGroup label="Updates">
        <Button
          variant="secondary"
          size="sm"
          className="self-start"
          disabled={state.status === 'checking'}
          onClick={check}
        >
          {state.status === 'checking' ? 'CHECKING...' : 'CHECK FOR UPDATES'}
        </Button>

        {state.status === 'current' && <SettingNote>You are up to date.</SettingNote>}
        {state.status === 'failed' && (
          <SettingNote tone="warn">Could not check: {state.detail}</SettingNote>
        )}
        {state.status === 'available' && (
          <>
            <SettingNote>Version {state.version} is available.</SettingNote>
            {/* Restarting is explicit: the user may be mid-set, and an app that
                relaunches itself under a playing track is unforgivable. */}
            <Button
              variant="primary"
              size="sm"
              className="self-start"
              disabled={state.installing}
              onClick={install}
            >
              {state.installing ? 'INSTALLING...' : 'INSTALL AND RESTART'}
            </Button>
          </>
        )}
      </SettingGroup>

      <SettingGroup label="Support">
        <button
          type="button"
          onClick={() => openExternal(COFFEE)}
          className="inline-flex items-center gap-2 self-start text-xs text-keystone-lime transition-colors hover:text-keystone-moss"
        >
          <Heart size={14} color="currentColor" variant="Bold" aria-hidden />
          <span>Buy me a coffee</span>
        </button>
      </SettingGroup>
    </SettingSection>
  );
}
