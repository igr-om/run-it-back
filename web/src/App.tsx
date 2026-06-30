import { Route, Routes } from "react-router-dom";
import ProtectedRoute from "./components/ProtectedRoute";
import Login from "./pages/Login";
import Register from "./pages/Register";
import Dashboard from "./pages/Dashboard";
import Trainer from "./pages/Trainer";
import RangeExplorer from "./pages/RangeExplorer";
import HandHistory from "./pages/HandHistory";
import About from "./pages/About";

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route path="/register" element={<Register />} />
      <Route
        path="/"
        element={
          <ProtectedRoute>
            <Dashboard />
          </ProtectedRoute>
        }
      />
      <Route
        path="/trainer"
        element={
          <ProtectedRoute>
            <Trainer />
          </ProtectedRoute>
        }
      />
      <Route
        path="/ranges"
        element={
          <ProtectedRoute>
            <RangeExplorer />
          </ProtectedRoute>
        }
      />
      <Route
        path="/hands"
        element={
          <ProtectedRoute>
            <HandHistory />
          </ProtectedRoute>
        }
      />
      <Route
        path="/about"
        element={
          <ProtectedRoute>
            <About />
          </ProtectedRoute>
        }
      />
    </Routes>
  );
}
