import { NavLink } from "react-router-dom";
import { useAuth } from "../store/auth";

const LINKS = [
  { to: "/", label: "Dashboard" },
  { to: "/trainer", label: "Drill Trainer" },
  { to: "/ranges", label: "Range Explorer" },
  { to: "/hands", label: "Hand History" },
  { to: "/about", label: "About / How it works" },
];

export default function NavBar() {
  const { user, logout } = useAuth();
  return (
    <nav className="sidebar">
      <div className="brand">
        <div className="brand-mark">RB</div>
        <div className="brand-name">Run It Back</div>
      </div>
      {LINKS.map((l) => (
        <NavLink key={l.to} to={l.to} className={({ isActive }) => "nav-link" + (isActive ? " active" : "")} end={l.to === "/"}>
          {l.label}
        </NavLink>
      ))}
      <div className="nav-spacer" />
      <div className="nav-footer">
        <div style={{ marginBottom: 8, color: "var(--text-dim)" }}>{user?.username}</div>
        <span className="nav-link" onClick={logout} style={{ padding: "6px 0" }}>
          Sign out
        </span>
      </div>
    </nav>
  );
}
