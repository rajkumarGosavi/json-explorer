import { createApp } from "vue";
import App from "./App.vue";
import { createPinia } from "pinia";
import router from "./router";

import "./style.css";
import PrimeVue from "primevue/config";
import Aura from "@primeuix/themes/aura";
import { definePreset } from "@primeuix/themes";
import "primeicons/primeicons.css";
import ToastService from "primevue/toastservice";

const app = createApp(App);

app.use(createPinia());
app.use(router);

const ExplorerPreset = definePreset(Aura, {
    semantic: {
        primary: {
            50: "#eef6fb",
            100: "#d5e8f4",
            200: "#aed1ea",
            300: "#7cb3db",
            400: "#4a90c7",
            500: "#2c6fa8",
            600: "#235a8a",
            700: "#1d4a71",
            800: "#1a3d5c",
            900: "#17334c",
            950: "#0d1f30",
        },
    },
});

app.use(PrimeVue, {
    theme: {
        preset: ExplorerPreset,
        options: { darkModeSelector: ".app-dark" },
    },
});
app.use(ToastService);

app.mount("#app");
