import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        surface: {
          "50": "#f8f9fc",
          "100": "#eef1f6",
          "200": "#dce1ec",
          "300": "#b8c1d4",
          "400": "#8d9ab5",
          "500": "#6b7a99",
          "600": "#556280",
          "700": "#454f68",
          "800": "#2d3548",
          "900": "#1a1f2e",
          "950": "#0f1219",
        },
        accent: {
          "50": "#edfcf5",
          "100": "#d3f8e6",
          "200": "#aaf0d1",
          "300": "#73e2b7",
          "400": "#3bce99",
          "500": "#17b37f",
          "600": "#0b9167",
          "700": "#097455",
          "800": "#0a5c44",
          "900": "#094b39",
          "950": "#042a21",
        },
        danger: {
          "400": "#f87171",
          "500": "#ef4444",
          "600": "#dc2626",
        },
        warning: {
          "400": "#fbbf24",
          "500": "#f59e0b",
        },
      },
      fontFamily: {
        sans: [
          '"Segoe UI"',
          '"Microsoft YaHei"',
          '"Noto Sans SC"',
          "system-ui",
          "-apple-system",
          "sans-serif",
        ],
        mono: ['"Cascadia Code"', '"Consolas"', '"JetBrains Mono"', '"Fira Code"', "monospace"],
      },
      animation: {
        "pulse-soft": "pulse-soft 2s ease-in-out infinite",
        "slide-in": "slide-in 0.3s ease-out",
        "fade-in": "fade-in 0.2s ease-out",
      },
      keyframes: {
        "pulse-soft": {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.6" },
        },
        "slide-in": {
          "0%": { transform: "translateX(-8px)", opacity: "0" },
          "100%": { transform: "translateX(0)", opacity: "1" },
        },
        "fade-in": {
          "0%": { opacity: "0" },
          "100%": { opacity: "1" },
        },
      },
    },
  },
  plugins: [],
};

export default config;
