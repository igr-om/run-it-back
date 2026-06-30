import { Navigate } from "react-router-dom";
import { useAuth } from "../store/auth";
import NavBar from "./NavBar";

export default function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();

  if (loading) {
    return (
      <div style={{ display: "flex", height: "100vh", alignItems: "center", justifyContent: "center" }}>
        <div className="spinner" />
      </div>
    );
  }
  if (!user) return <Navigate to="/login" replace />;

  return (
    <div className="app-shell">
      <NavBar />
      <main className="main">{children}</main>
    </div>
  );
}
