export interface User {
  id: string;
  email: string;
  display_name: string;
  has_startgg_key: boolean;
  created_at: string;
}

export interface Project {
  id: string;
  name: string;
  game_id: number | null;
  game_name: string | null;
  created_at: string;
  published: boolean;
  user_role: "owner" | "editor" | "viewer" | null;
  owner_has_startgg_key: boolean;
}

export interface Account {
  id: string;
  startgg_user_id: number;
  handle: string;
  display_name: string | null;
}

export interface Player {
  id: string;
  project_id: string;
  name: string;
  rank_position: number;
  created_at: string;
  accounts: Account[];
}

export interface TournamentParticipant {
  startgg_user_id: number;
  handle: string;
  name: string;
}

export interface TournamentEntrantOrdered {
  startgg_user_id: number;
  handle: string;
  name: string;
  seed: number | null;
  placement: number | null;
}

export interface TournamentEventData {
  id: number;
  name: string;
  state: string | null;
  entrants: TournamentEntrantOrdered[];
}

export interface TournamentData {
  all_participants: TournamentParticipant[];
  events: TournamentEventData[];
}

export interface BulkAddResult {
  name: string;
  handle: string;
  status: "created" | "skipped";
}

export interface ByHandlesResult {
  handle: string;
  name: string | null;
  status: "created" | "skipped" | "not_found";
}

export interface TournamentEvent {
  id: string;
  startgg_id: number;
  name: string;
  game_name: string | null;
  num_entrants: number | null;
  start_at: string | null;
  included: boolean;
  event_type: number | null;
  bracket_types: string[];
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

export interface ImportProgress {
  phase: "scanning" | "importing";
  step: number;
  total: number;
}

export interface Job {
  id: string;
  status: "pending" | "running" | "done" | "failed";
  error: string | null;
  after_date: string | null;
  before_date: string | null;
  created_at: string;
  updated_at: string;
  progress: ImportProgress | null;
}

export interface SetRecord {
  opponent_id: string;
  opponent_name: string;
  upset_factor: number;
  winner_score: number | null;
  loser_score: number | null;
  tournament_name: string;
  tournament_handle: string;
  event_name: string;
  round_name: string | null;
  completed_at: string | null;
  is_dq: boolean;
  vod_url: string | null;
  startgg_set_id: number;
  winner_seed: number | null;
  loser_seed: number | null;
  phase_name: string | null;
  pool_identifier: string | null;
  winner_placement: number | null;
  loser_placement: number | null;
  location: string | null;
  num_entrants: number | null;
  event_handle: string | null;
}

export interface H2HSet extends SetRecord {
  is_win: boolean;
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

export interface ProjectMember {
  project_id: string;
  user_id: string;
  display_name: string;
  email: string;
  role: "editor" | "viewer";
  joined_at: string;
}

export interface InviteLink {
  id: string;
  project_id: string;
  role: "editor" | "viewer";
  created_by: string;
  expires_at: string | null;
  revoked_at: string | null;
  created_at: string;
}

export interface AcceptInviteResponse {
  project_id: string;
}

export interface TournamentAttendance {
  tournament_name: string;
  tournament_slug: string;
  event_name: string;
  placement: number | null;
  num_entrants: number | null;
  start_at: string | null;
  location: string | null;
}
