import type {
  AuthResponse,
  DrillAttemptRecord,
  DrillView,
  GradeResult,
  HandHistoryRecord,
  PlayerStats,
  RangeRecord,
  SolveJob,
  SolveResponse,
  User,
  WeaknessProfile,
} from "./types";

const BASE = "/api";

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

function getToken(): string | null {
  return localStorage.getItem("rib_token");
}

export function setToken(token: string | null) {
  if (token) localStorage.setItem("rib_token", token);
  else localStorage.removeItem("rib_token");
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string> | undefined),
  };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(`${BASE}${path}`, { ...options, headers });
  if (!res.ok) {
    let message = `request failed (${res.status})`;
    try {
      const body = await res.json();
      if (body?.error) message = body.error;
    } catch {
      // ignore non-JSON error bodies
    }
    throw new ApiError(res.status, message);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const api = {
  register: (username: string, email: string, password: string) =>
    request<AuthResponse>("/auth/register", { method: "POST", body: JSON.stringify({ username, email, password }) }),
  login: (username: string, password: string) =>
    request<AuthResponse>("/auth/login", { method: "POST", body: JSON.stringify({ username, password }) }),
  me: () => request<User>("/me"),

  listRanges: () => request<RangeRecord[]>("/ranges"),
  createRange: (name: string, game_type: string, range_string: string) =>
    request<RangeRecord>("/ranges", { method: "POST", body: JSON.stringify({ name, game_type, range_string }) }),
  deleteRange: (id: string) => request<{ deleted: boolean }>(`/ranges/${id}`, { method: "DELETE" }),

  enqueueSolve: (req: unknown) =>
    request<{ job_id: string; status: string }>("/solve", { method: "POST", body: JSON.stringify(req) }),
  solveJobStatus: (id: string) => request<SolveJob>(`/solve/jobs/${id}`),
  preflopLibrary: (hero: string, villain: string, stackBb: number, potType: string) =>
    request<SolveResponse>(`/solve/preflop?hero=${hero}&villain=${villain}&stack_bb=${stackBb}&pot_type=${potType}`),

  generateDrill: () => request<DrillView>("/drills/generate", { method: "POST" }),
  answerDrill: (id: string, chosenActionIndex: number) =>
    request<GradeResult>(`/drills/${id}/answer`, {
      method: "POST",
      body: JSON.stringify({ chosen_action_index: chosenActionIndex }),
    }),
  recentAttempts: () => request<DrillAttemptRecord[]>("/drills/attempts"),
  weaknessProfile: () => request<WeaknessProfile[]>("/stats/weakness"),

  uploadHandHistory: (raw_text: string, filename?: string, site_hint?: string) =>
    request<{ hand_history_id: string; status: string }>("/hands/upload", {
      method: "POST",
      body: JSON.stringify({ raw_text, filename, site_hint }),
    }),
  listHandHistories: () => request<HandHistoryRecord[]>("/hands"),
  statsOverview: () => request<PlayerStats | null>("/stats/overview"),
};

/** Polls a solve job until it's done/failed, calling `onTick` with each poll. */
export async function pollSolveJob(id: string, onTick?: (job: SolveJob) => void, timeoutMs = 30000): Promise<SolveJob> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const job = await api.solveJobStatus(id);
    onTick?.(job);
    if (job.status === "done" || job.status === "failed") return job;
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new ApiError(408, "solve timed out -- the spot may be unusually large, try narrowing the ranges");
}
