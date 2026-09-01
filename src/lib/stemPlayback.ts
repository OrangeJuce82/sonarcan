export interface StemPlaybackResumeRequest {
  selectionGeneration: number;
  trackId: string;
}

export function stemPlaybackResumeRequest(
  shouldResume: boolean,
  trackId: string,
  selectionGeneration: number,
): StemPlaybackResumeRequest | null {
  return shouldResume ? { trackId, selectionGeneration } : null;
}

export function shouldResumeStemPlayback(
  request: StemPlaybackResumeRequest | null,
  currentTrackId: string | undefined,
  selectionGeneration: number,
): boolean {
  return request !== null
    && request.trackId === currentTrackId
    && request.selectionGeneration === selectionGeneration;
}
