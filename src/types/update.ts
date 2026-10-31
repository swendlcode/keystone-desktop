/**
 * What `check_for_update` reports. Mirrors `commands::update_commands::UpdateInfo`.
 *
 * `latestVersion` stays null until the updater plugin has fetched and verified
 * the signed manifest: there is deliberately no second, unsigned path to "is
 * there an update", because that would be a network call with none of the
 * signature checking that makes the real one safe.
 */
export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string | null;
  updateAvailable: boolean;
  notes: string | null;
}
