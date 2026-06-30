export type RawAction = "fold" | "check" | "call" | { bet: number } | { raise: number } | { all_in: number };

export function actionLabel(a: RawAction): string {
  if (a === "fold") return "Fold";
  if (a === "check") return "Check";
  if (a === "call") return "Call";
  if ("bet" in a) return `Bet ${a.bet.toFixed(1)}bb`;
  if ("raise" in a) return `Raise to ${a.raise.toFixed(1)}bb`;
  if ("all_in" in a) return `All-in ${a.all_in.toFixed(1)}bb`;
  return "?";
}

export function actionIsAggressive(a: RawAction): boolean {
  return typeof a !== "string";
}

export interface User {
  id: string;
  username: string;
  email: string;
  created_at: string;
  last_login_at: string | null;
}

export interface AuthResponse {
  token: string;
  user: User;
}

export interface Strategy {
  actions: RawAction[];
  frequencies: Record<string, number[]>;
  ev_bb: Record<string, number>;
}

export interface SolveResponse {
  hero_strategy: Strategy;
  action_ev_bb: Record<string, number[]>;
  hero_ev_bb: number;
  villain_ev_bb: number;
  iterations_run: number;
  exploitability_estimate: number;
  n_hero_combos: number;
  n_villain_combos: number;
  warnings: string[];
}

export interface SolveJob {
  id: string;
  user_id: string | null;
  request: unknown;
  status: "queued" | "running" | "done" | "failed";
  progress: number;
  result: SolveResponse | null;
  error: string | null;
  created_at: string;
  completed_at: string | null;
}

export interface DrillView {
  id: string;
  category: string;
  spot_snapshot: Record<string, unknown>;
  dealt_hand: string[];
  available_actions: RawAction[];
}

export interface GradeResult {
  is_correct: boolean;
  chosen_action: string;
  best_action: string;
  chosen_ev_bb: number;
  best_ev_bb: number;
  ev_loss_bb: number;
  explanation: string;
}

export interface WeaknessProfile {
  id: string;
  user_id: string;
  game_type: string;
  category: string;
  attempts: number;
  correct: number;
  avg_ev_loss_bb: number;
  last_seen_at: string;
}

export interface PlayerStats {
  id: string;
  user_id: string;
  game_type: string;
  sample_size: number;
  vpip: number | null;
  pfr: number | null;
  three_bet: number | null;
  fold_to_three_bet: number | null;
  cbet_flop: number | null;
  fold_to_cbet_flop: number | null;
  cbet_turn: number | null;
  wtsd: number | null;
  won_at_showdown: number | null;
  aggression_factor: number | null;
  net_bb_per_100: number | null;
  updated_at: string;
}

export interface HandHistoryRecord {
  id: string;
  user_id: string;
  site: string;
  original_filename: string | null;
  hand_count: number;
  status: string;
  error: string | null;
  uploaded_at: string;
  parsed_at: string | null;
}

export interface RangeRecord {
  id: string;
  user_id: string | null;
  name: string;
  game_type: string;
  range_string: string;
  created_at: string;
}

export interface DrillAttemptRecord {
  id: string;
  drill_id: string;
  user_id: string;
  chosen_action: string;
  ev_loss_bb: number;
  is_correct: boolean;
  explanation: string;
  answered_at: string;
}

export const CATEGORY_LABELS: Record<string, string> = {
  open_raise: "Preflop Open",
  vs_open_defend: "Defending vs. an Open",
  vs_three_bet: "Facing a 3-Bet",
  cbet_flop: "Flop C-Bet",
  vs_cbet_flop: "Facing a Flop C-Bet",
  turn_barrel: "Turn Barrel",
  vs_turn_barrel: "Facing a Turn Barrel",
  river_bet: "River Bet",
  vs_river_bet: "Facing a River Bet",
};
