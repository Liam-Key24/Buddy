import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./AppShell";
import { AuthGate } from "./AuthGate";
import { CalendarPage } from "./pages/CalendarPage";
import { ChatPage } from "./pages/ChatPage";
import { GoalsPage } from "./pages/GoalsPage";
import { SparksPage } from "./pages/SparksPage";
import { TodayPage } from "./pages/TodayPage";
import { AboutSettingsPage } from "./pages/settings/AboutSettingsPage";
import { CalendarSettingsPage } from "./pages/settings/CalendarSettingsPage";
import { GeneralSettingsPage } from "./pages/settings/GeneralSettingsPage";
import { SettingsLayout } from "./pages/settings/SettingsLayout";
import "./index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <AuthGate>
      <Routes>
        <Route element={<AppShell />}>
          <Route path="/" element={<TodayPage />} />
          <Route path="/chat" element={<ChatPage />} />
          <Route path="/goals" element={<GoalsPage />} />
          <Route path="/calendar" element={<CalendarPage />} />
          <Route path="/sparks" element={<SparksPage />} />
          <Route path="/settings" element={<SettingsLayout />}>
            <Route index element={<Navigate to="about" replace />} />
            <Route path="about" element={<AboutSettingsPage />} />
            <Route path="general" element={<GeneralSettingsPage />} />
            <Route path="calendar" element={<CalendarSettingsPage />} />
          </Route>
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
      </AuthGate>
    </BrowserRouter>
  </StrictMode>,
);
