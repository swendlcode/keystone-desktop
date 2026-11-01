/** One importable playlist from the Apple Music library XML. */
export interface MusicPlaylist {
  name: string;
  /**
   * Entries that resolve to a local audio file that exists right now — not
   * the playlist's length. An Apple Music subscription playlist is mostly
   * streaming entries with no file behind them, and showing 173 next to an
   * import that adds four is worse than showing nothing.
   */
  trackCount: number;
}

export interface MusicLibraryInfo {
  /** Where the XML was read from, or null when Music is not publishing one. */
  xmlPath: string | null;
  playlists: MusicPlaylist[];
}
