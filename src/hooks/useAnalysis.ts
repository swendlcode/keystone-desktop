import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../services/keystoneService';
import { useAnalysisStore } from '../stores/analysisStore';
import { trackKeys } from './useLibrary';
import {
  KeystoneEvent,
  type AnalysisProgressEvent,
  type QueueFinishedEvent,
  type WaveformReadyEvent,
} from '../types';

export const analysisKeys = {
  all: ['analysis'] as const,
  waveform: (trackId: string) =>
    [...analysisKeys.all, 'waveform', trackId] as const,
  regions: (trackId: string) =>
    [...analysisKeys.all, 'regions', trackId] as const,
  detail: (trackId: string) => [...analysisKeys.all, 'detail', trackId] as const,
};

/**
 * Queue events. Mounted once, from App.
 *
 * Progress arrives on a timer rather than per track, which is why it writes
 * straight into a Zustand store: pushing it through react-query would mean a
 * cache write and a re-render chain per tick for a number that is only ever
 * displayed, never queried.
 */
export function useAnalysisSync() {
  const qc = useQueryClient();
  const setProgress = useAnalysisStore((s) => s.setProgress);
  const setLastRun = useAnalysisStore((s) => s.setLastRun);

  useEffect(() => {
    const unsubs: Array<Promise<() => void>> = [];

    unsubs.push(
      listen<AnalysisProgressEvent>(KeystoneEvent.analysisProgress, (e) => {
        setProgress(e.payload.progress);
      }),
    );

    unsubs.push(
      listen<QueueFinishedEvent>(KeystoneEvent.queueFinished, (e) => {
        setLastRun(e.payload);
        // The rows were patched one by one off `track-updated`; this catches
        // the aggregate reads — wheel counts, genre list, collection counts.
        qc.invalidateQueries({ queryKey: trackKeys.all });
        qc.invalidateQueries({ queryKey: ['collections'] });
      }),
    );

    unsubs.push(
      listen<WaveformReadyEvent>(KeystoneEvent.waveformReady, (e) => {
        // Only invalidate: the payload carries the hash, not the peaks, which
        // are megabytes and belong in a fetch the viewer actually asked for.
        qc.invalidateQueries({
          queryKey: analysisKeys.waveform(e.payload.trackId),
        });
      }),
    );

    return () => {
      Promise.all(unsubs).then((fns) => fns.forEach((fn) => fn()));
    };
  }, [qc, setProgress, setLastRun]);
}

/** Current queue state. Polled once on mount; the event stream does the rest. */
export function useAnalysisProgress() {
  const progress = useAnalysisStore((s) => s.progress);
  const setProgress = useAnalysisStore((s) => s.setProgress);

  const query = useQuery({
    queryKey: [...analysisKeys.all, 'progress'],
    queryFn: () => keystoneService.getAnalysisProgress(),
    staleTime: Infinity,
  });

  useEffect(() => {
    if (query.data && !progress) setProgress(query.data);
    // Only seeds the store; afterwards the event stream is the source.
  }, [query.data, progress, setProgress]);

  return progress ?? query.data ?? null;
}

export function useWaveform(trackId: string | null) {
  return useQuery({
    queryKey: trackId
      ? analysisKeys.waveform(trackId)
      : [...analysisKeys.all, 'waveform', 'none'],
    queryFn: () => (trackId ? keystoneService.getWaveform(trackId) : null),
    enabled: Boolean(trackId),
    // Peaks are keyed by content hash and never change for a given file, so
    // once fetched they do not need re-validating.
    staleTime: Infinity,
  });
}

export function useRegions(trackId: string | null) {
  return useQuery({
    queryKey: trackId
      ? analysisKeys.regions(trackId)
      : [...analysisKeys.all, 'regions', 'none'],
    queryFn: () => (trackId ? keystoneService.getRegions(trackId) : []),
    enabled: Boolean(trackId),
  });
}

export function useAnalysisDetail(trackId: string | null) {
  return useQuery({
    queryKey: trackId
      ? analysisKeys.detail(trackId)
      : [...analysisKeys.all, 'detail', 'none'],
    queryFn: () => (trackId ? keystoneService.getAnalysis(trackId) : null),
    enabled: Boolean(trackId),
  });
}

export function useEnqueueTracks() {
  const setProgress = useAnalysisStore((s) => s.setProgress);
  return useMutation({
    mutationFn: (trackIds: string[]) =>
      keystoneService.enqueueTracks(trackIds),
    onSuccess: setProgress,
  });
}

export function useReanalyze() {
  const setProgress = useAnalysisStore((s) => s.setProgress);
  return useMutation({
    mutationFn: (trackIds: string[]) =>
      keystoneService.reanalyzeTracks(trackIds),
    onSuccess: setProgress,
  });
}

export function useAnalyzeAll() {
  const setProgress = useAnalysisStore((s) => s.setProgress);
  return useMutation({
    mutationFn: () => keystoneService.enqueueAllUnanalyzed(),
    onSuccess: setProgress,
  });
}

/** Pause / resume / clear, as the sidebar's queue controls. */
export function useQueueControls() {
  const setProgress = useAnalysisStore((s) => s.setProgress);
  const pause = useMutation({
    mutationFn: () => keystoneService.pauseQueue(),
    onSuccess: setProgress,
  });
  const resume = useMutation({
    mutationFn: () => keystoneService.resumeQueue(),
    onSuccess: setProgress,
  });
  const clear = useMutation({
    mutationFn: () => keystoneService.clearQueue(),
    onSuccess: setProgress,
  });
  return { pause, resume, clear };
}
