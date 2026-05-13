export interface User {
	id: string;
	username: string;
	created_at: string;
}

export interface Project {
	id: string;
	name: string;
	game_id: number | null;
	game_name: string | null;
	created_at: string;
}

export interface Account {
	id: string;
	startgg_user_id: number;
	slug: string;
	display_name: string | null;
}

export interface Player {
	id: string;
	project_id: string;
	name: string;
	created_at: string;
	accounts: Account[];
}

export interface TournamentEvent {
	id: string;
	startgg_id: number;
	name: string;
	game_name: string | null;
	num_entrants: number | null;
	start_at: string | null;
	included: boolean;
}

export interface Tournament {
	id: string;
	startgg_id: number;
	name: string;
	slug: string;
	city: string | null;
	addr_state: string | null;
	country_code: string | null;
	venue_name: string | null;
	online: boolean;
	start_at: string | null;
	end_at: string | null;
	events: TournamentEvent[];
}

export interface Job {
	id: string;
	status: 'pending' | 'running' | 'done' | 'failed';
	error: string | null;
	after_date: string | null;
	before_date: string | null;
	created_at: string;
	updated_at: string;
}

export interface SetRecord {
	opponent_id: string;
	opponent_name: string;
	upset_factor: number;
	winner_score: number | null;
	loser_score: number | null;
}

export interface PlayerStats {
	player_id: string;
	name: string;
	wins: SetRecord[];
	losses: SetRecord[];
}

export interface HeadToHeadEntry {
	player_id: string;
	opponent_id: string;
	wins: number;
	losses: number;
}

export interface Game {
	id: number;
	name: string;
	display_name: string | null;
}
