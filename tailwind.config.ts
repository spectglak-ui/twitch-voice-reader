import type { Config } from "tailwindcss";

// --- Système de design "Twitch Voice Reader" ---------------------------
// Palette dérivée du logo fourni (bulle lavande + glyphes violets) plutôt
// que d'un violet Discord générique : un fond quasi noir à teinte violette
// (jamais un vrai #000, pour éviter l'effet "écran cassé" en usage
// prolongé), avec quatre accents nommés portant chacun un sens fonctionnel
// distinct (jamais de couleur décorative sans rôle) :
//   - `signal`  (violet, identité de marque)   : actions primaires, focus
//   - `live`    (vert)                         : connecté / en lecture
//   - `caution` (ambre)                        : file d'attente, latence
//   - `alert`   (rouge)                        : déconnecté / erreur
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        base: {
          950: "#0B0912",
          900: "#120F1C",
          800: "#181426",
          700: "#221C35",
          600: "#2E2645",
          border: "#2A2440",
        },
        signal: {
          DEFAULT: "#7C5CFC",
          bright: "#9B7BFF",
          dim: "#4C3A99",
        },
        live: {
          DEFAULT: "#34D399",
          dim: "#1D6E52",
        },
        caution: {
          DEFAULT: "#F5A524",
          dim: "#8A5D14",
        },
        alert: {
          DEFAULT: "#F43F5E",
          dim: "#7A2030",
        },
        ink: {
          DEFAULT: "#F1EEFB",
          muted: "#B7AFD1",
          faint: "#7A7195",
        },
      },
      fontFamily: {
        display: ["'Space Grotesk'", "sans-serif"],
        body: ["'Inter'", "sans-serif"],
        mono: ["'JetBrains Mono'", "monospace"],
      },
      borderRadius: {
        panel: "10px",
      },
      keyframes: {
        "waveform-bar": {
          "0%, 100%": { transform: "scaleY(0.3)" },
          "50%": { transform: "scaleY(1)" },
        },
        "pulse-dot": {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.4" },
        },
      },
      animation: {
        "waveform-bar": "waveform-bar 0.8s ease-in-out infinite",
        "pulse-dot": "pulse-dot 1.6s ease-in-out infinite",
      },
    },
  },
  plugins: [],
} satisfies Config;
