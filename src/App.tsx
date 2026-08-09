import { HashRouter, Routes, Route } from "react-router-dom";
import { useEffect } from "react";
import { AppLayout } from "@/components/layout/AppLayout";
import { Dashboard } from "@/pages/Dashboard";
import { Connections } from "@/pages/Connections";
import { Voice } from "@/pages/Voice";
import { Filters } from "@/pages/Filters";
import { History } from "@/pages/History";
import { Settings } from "@/pages/Settings";
import { useChatStore } from "@/store/chatStore";
import { useConnectionStore } from "@/store/connectionStore";

// `HashRouter` plutôt que `BrowserRouter` : l'application est servie
// depuis le protocole `tauri://` en production, où l'historique HTML5
// (basé sur des chemins réels) n'est pas pertinent — le hash routing évite
// toute configuration serveur supplémentaire.
export default function App() {
  const initChat = useChatStore((s) => s.init);
  const initConnections = useConnectionStore((s) => s.init);

  useEffect(() => {
    // Écoutes d'évènements globales démarrées une seule fois au montage de
    // l'application, indépendamment de la page affichée, afin que le flux
    // de chat et l'état des connexions restent à jour même en arrière-plan.
    initChat();
    initConnections();
  }, [initChat, initConnections]);

  return (
    <HashRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<Dashboard />} />
          <Route path="connections" element={<Connections />} />
          <Route path="voice" element={<Voice />} />
          <Route path="filters" element={<Filters />} />
          <Route path="history" element={<History />} />
          <Route path="settings" element={<Settings />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
