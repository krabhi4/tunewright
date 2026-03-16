export type AudioFormat = 'mp3' | 'flac' | 'mp4' | 'ogg' | 'opus' | 'wav' | 'aiff';

export interface FileEntry {
	id: string;
	filename: string;
	relative_path: string;
	format: AudioFormat;
	size: number;
	duration_secs: number | null;
	has_cover: boolean;
	modified_at: string;
}

export interface TagData {
	title?: string;
	artist?: string;
	album?: string;
	album_artist?: string;
	year?: number;
	track_number?: number;
	track_total?: number;
	disc_number?: number;
	disc_total?: number;
	genre?: string;
	comment?: string;
	composer?: string;
	bitrate?: number;
	sample_rate?: number;
	channels?: number;
	duration_secs?: number;
	format?: string;
	tag_types?: string[];
}

export interface FileListResult {
	path: string;
	files: FileEntry[];
	total: number;
	directories: string[];
}

export interface DirNode {
	name: string;
	path: string;
	children: DirNode[];
}
