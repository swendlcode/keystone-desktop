import { useState } from 'react';
import { Button } from '../ui/Button';
import { Heart, Logo } from '../ui/icons';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { openExternal, useAppVersion, useUpdateCheck } from './useAppInfo';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { appDataDir, join } from '@tauri-apps/api/path';
import { versionLabel } from '../../utils/appBuild';

const COFFEE = 'https://www.buymeacoffee.com/swendl';

export function AboutPane() {
  const { data: version } = useAppVersion();
  const { state, check, install } = useUpdateCheck();
  // `revealItemInDir` rejects when the log does not exist yet, which on a
  // healthy install is most of the time. Swallowing that made the button
  // indistinguishable from a broken one: the user cannot tell "nothing has
  // gone wrong yet" from "this control is dead".
  const [noLogYet, setNoLogYet] = useState(false);

  return (
    <SettingSection title="About">
      <SettingGroup>
        <Logo size={22} />
        <p className="mono mt-2 text-[12px] text-gray-400">
          Version {version ? versionLabel(version) : '—'}
        </p>
        <p className="mono text-[12px] text-gray-500">
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

      <SettingGroup label="Diagnostics">
        {/* A record that is written automatically and cannot be found is half
            a record: the file sits beside the database, which is not
            somewhere anybody would think to look. */}
        <Button
          variant="secondary"
          size="sm"
          className="self-start"
          onClick={() => {
            void appDataDir()
              .then((dir) => join(dir, 'client-log.txt'))
              .then(revealItemInDir)
              .then(() => setNoLogYet(false))
              .catch(() => setNoLogYet(true));
          }}
        >
          SHOW LOG FILE
        </Button>
        <SettingNote>
          {noLogYet
            ? 'Nothing recorded yet — no playback has failed on this machine.'
            : 'Written when playback fails, or when a track takes too long to start.'}
        </SettingNote>
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
