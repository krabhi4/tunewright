import { apiFetch } from './client';

export interface ReleaseSearchResult {
	id: string;
	title: string;
	artist: string;
	year: number | null;
	track_count: number | null;
	source: string;
	cover_art_url: string | null;
}

export interface TrackInfo {
	position: number;
	title: string;
	artist: string | null;
	duration_secs: number | null;
}

export interface ReleaseDetail {
	id: string;
	title: string;
	artist: string;
	year: number | null;
	genre: string | null;
	tracks: TrackInfo[];
	source: string;
	cover_art_url: string | null;
}

export async function searchMusicBrainz(query: string): Promise<ReleaseSearchResult[]> {
	const params = new URLSearchParams({ query });
	return apiFetch<ReleaseSearchResult[]>(`/lookup/musicbrainz/search?${params}`);
}

export async function getMusicBrainzRelease(mbid: string): Promise<ReleaseDetail> {
	return apiFetch<ReleaseDetail>(`/lookup/musicbrainz/release/${mbid}`);
}

export async function searchAppleMusic(query: string): Promise<ReleaseSearchResult[]> {
	const params = new URLSearchParams({ query });
	return apiFetch<ReleaseSearchResult[]>(`/lookup/applemusic/search?${params}`);
}

export async function getAppleMusicRelease(id: string): Promise<ReleaseDetail> {
	return apiFetch<ReleaseDetail>(`/lookup/applemusic/release/${id}`);
}
