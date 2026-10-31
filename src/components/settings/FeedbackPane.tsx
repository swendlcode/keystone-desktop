import { useState } from 'react';
import { Button } from '../ui/Button';
import { Heart } from '../ui/icons';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { openExternal } from './useAppInfo';

const MAILTO = 'mailto:hello@swendl.com?subject=Keystone%20feedback&body=';
const COFFEE = 'https://www.buymeacoffee.com/swendl';

/**
 * Send Feedback.
 *
 * It composes a mail rather than posting anywhere. Keystone makes no network
 * call of its own by design — the engine plan forbids one in the analysis path
 * and there is no server to receive this — so the honest implementation hands
 * the text to the user's own mail client, where they can see what is being sent.
 */
export function FeedbackPane() {
  const [text, setText] = useState('');

  return (
    <SettingSection title="Send Feedback">
      <SettingGroup>
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          rows={6}
          placeholder="What went wrong, or what is missing?"
          className="w-full resize-none rounded-md border border-gray-600 bg-gray-700 p-3 text-xs text-keystone-white outline-none placeholder:text-gray-400 focus:border-keystone-lime"
        />
        <div className="flex items-center justify-between gap-3">
          <SettingNote>Opens your mail app. Nothing is sent from here.</SettingNote>
          <Button
            variant="primary"
            size="sm"
            disabled={!text.trim()}
            onClick={() => openExternal(MAILTO + encodeURIComponent(text))}
          >
            SEND
          </Button>
        </div>
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
